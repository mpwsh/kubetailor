use kubetailor::{
    crd::{self, Expose},
    k8s_openapi::{
        api::{
            apps::v1::DeploymentSpec,
            core::v1::{
                ConfigMapEnvSource, ConfigMapVolumeSource, Container, ContainerPort,
                EmptyDirVolumeSource, EnvFromSource, EnvVar, PersistentVolumeClaimVolumeSource,
                PodSpec, PodTemplateSpec, ResourceRequirements, SecretEnvSource, SecurityContext,
                Volume, VolumeMount,
            },
        },
        apimachinery::pkg::api::resource::Quantity,
    },
};

use crate::prelude::*;

/// Well-known topology label every flint node carries.
pub const REGION_LABEL: &str = "topology.kubernetes.io/region";

const GIT_SYNC_DEST: &str = "git-sync";
const GIT_SYNC_ROOT: &str = "/tmp/git";

/// Region pinning: with one node per region this selects "the" node; with several it lets the
/// scheduler pick one whose host ports are free.
pub fn node_selector(deployment: &crd::Deployment) -> Option<BTreeMap<String, String>> {
    deployment
        .region
        .as_ref()
        .map(|region| BTreeMap::from([(REGION_LABEL.to_owned(), region.to_owned())]))
}

/// Requests equal to what the app declared, and a memory limit at the same value: the scheduler
/// counts requests when deciding a node is full, and a memory limit keeps one app from taking
/// the node down with it. CPU is left unlimited so an idle neighbour's share is usable.
pub fn resource_requirements(container: &crd::Container) -> Option<ResourceRequirements> {
    let r = container.resources.as_ref()?;
    let requests = BTreeMap::from([
        ("cpu".to_owned(), Quantity(r.cpu.clone())),
        ("memory".to_owned(), Quantity(r.memory.clone())),
    ]);
    let limits = BTreeMap::from([("memory".to_owned(), Quantity(r.memory.clone()))]);
    Some(ResourceRequirements {
        requests: Some(requests),
        limits: Some(limits),
        ..ResourceRequirements::default()
    })
}

/// The HTTP port (if any) followed by the extra ports. `expose: node` ports bind the same number
/// on the node; everything else is a plain container port.
pub fn container_ports(container: &crd::Container) -> Vec<ContainerPort> {
    let mut ports = Vec::new();
    if let Some(http) = container.port {
        ports.push(ContainerPort {
            name: Some("http".to_owned()),
            container_port: http,
            protocol: Some("TCP".to_owned()),
            ..ContainerPort::default()
        });
    }
    for p in &container.ports {
        ports.push(ContainerPort {
            name: Some(p.name()),
            container_port: p.port,
            protocol: Some(p.protocol.as_str().to_owned()),
            host_port: (p.expose == Expose::Node).then_some(p.port),
            ..ContainerPort::default()
        });
    }
    ports
}

/// What backs a mount in the app container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MountSource {
    /// A ConfigMap holding the files of one directory.
    ConfigMap(String),
    /// A PersistentVolumeClaim.
    Pvc(String),
}

/// Something mounted into the app container: a whole volume at a directory, or one key of a
/// ConfigMap volume at a file path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    /// Pod volume name (a DNS label, 63 chars max, so not the resource name itself). Several
    /// mounts may share one volume; the pod gets it once.
    pub volume: String,
    /// Mount path inside the container.
    pub path: String,
    pub source: MountSource,
    /// Mount only this entry of the volume (`subPath`). A file is projected over the one path
    /// and the rest of its directory stays what the image put there; a plain mount of a
    /// ConfigMap at `/app` would hide `/app/server` and everything else in it.
    pub sub_path: Option<String>,
}

/// Pod template annotation carrying a digest of everything the container reads at start-up
/// (env, secrets, files): when any of it changes the digest changes, the template changes, and
/// the Deployment rolls the pods. Env vars are only read at start, and a `subPath` file mount
/// is not updated in place by the kubelet, so without this an edit would land in the ConfigMap
/// and never reach a running container.
pub const CONFIG_DIGEST_ANNOTATION: &str = "kubetailor.io/config-digest";

/// FNV-1a over a canonical serialisation of env, secrets and files.
pub fn config_digest(app: &TailoredApp) -> String {
    let canonical = serde_json::json!({
        "env": app.spec.env,
        "secrets": app.spec.secrets,
        "files": app.spec.deployment.container.files,
    })
    .to_string();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in canonical.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

pub fn new(meta: &TappMeta, app: &TailoredApp, mounts: &[Mount]) -> Deployment {
    let deployment = app.spec.deployment.clone();
    let mut containers = Vec::new();
    let mut volume_mounts = Vec::new();
    let mut pod_volumes = Vec::new();

    let init_mount = VolumeMount {
        name: String::from("init-vol"),
        mount_path: "/init".to_owned(),
        ..VolumeMount::default()
    };
    let init_vol = Volume {
        name: String::from("init-vol"),
        empty_dir: Some(EmptyDirVolumeSource {
            ..EmptyDirVolumeSource::default()
        }),
        ..Volume::default()
    };
    pod_volumes.push(init_vol);
    volume_mounts.push(init_mount.clone());
    let init_container = Container {
        name: "init".to_owned(),
        image: Some("kubetailor/init:latest".to_owned()),
        image_pull_policy: Some("IfNotPresent".to_owned()),
        volume_mounts: Some(vec![init_mount]),
        env: Some(vec![
            EnvVar {
                name: "BUILD_COMMAND".to_owned(),
                value: deployment.container.build_command,
                value_from: None,
            },
            EnvVar {
                name: "RUN_COMMAND".to_owned(),
                value: deployment.container.run_command.clone(),
                value_from: None,
            },
            EnvVar {
                name: "CONTAINER_IMAGE".to_owned(),
                value: Some(deployment.container.image.to_owned()),
                value_from: None,
            },
        ]),
        ..Container::default()
    };
    let command = if app.spec.git.is_some() {
        Some(vec!["/init/run.sh".to_string()])
    } else {
        deployment
            .container
            .run_command
            .map(|run_cmd| vec![run_cmd])
    };
    let mut env_from = Vec::new();

    if app.spec.env.is_some() {
        env_from.push(EnvFromSource {
            config_map_ref: Some(ConfigMapEnvSource {
                name: Some(meta.name.to_owned()),
                ..ConfigMapEnvSource::default()
            }),
            secret_ref: None,
            ..EnvFromSource::default()
        });
    }

    if app.spec.secrets.is_some() {
        env_from.push(EnvFromSource {
            config_map_ref: None,
            secret_ref: Some(SecretEnvSource {
                name: Some(meta.name.to_owned()),
                ..SecretEnvSource::default()
            }),
            ..EnvFromSource::default()
        });
    }

    let mut container = Container {
        name: "app".to_owned(), //meta.name.to_owned(),
        image: Some(deployment.container.image.to_owned()),
        image_pull_policy: Some("IfNotPresent".to_owned()),
        command,
        ports: Some(container_ports(&app.spec.deployment.container)),
        resources: resource_requirements(&app.spec.deployment.container),
        env_from: if !env_from.is_empty() {
            Some(env_from)
        } else {
            None
        },
        //CAP_NET_BIND_SERVICE
        security_context: Some(SecurityContext {
            allow_privilege_escalation: app.spec.deployment.allow_privilege_escalation,
            run_as_non_root: app.spec.deployment.allow_root.map(|root| !root),
            run_as_user: app.spec.deployment.run_as_user,
            run_as_group: app.spec.deployment.run_as_group,
            ..SecurityContext::default()
        }),
        ..Container::default()
    };

    for m in mounts {
        volume_mounts.push(VolumeMount {
            name: m.volume.clone(),
            mount_path: m.path.clone(),
            sub_path: m.sub_path.clone(),
            ..VolumeMount::default()
        });
        if pod_volumes.iter().any(|v: &Volume| v.name == m.volume) {
            continue;
        }
        pod_volumes.push(match &m.source {
            MountSource::ConfigMap(name) => Volume {
                name: m.volume.clone(),
                config_map: Some(ConfigMapVolumeSource {
                    name: Some(name.clone()),
                    ..ConfigMapVolumeSource::default()
                }),
                ..Volume::default()
            },
            MountSource::Pvc(name) => Volume {
                name: m.volume.clone(),
                persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                    claim_name: name.clone(),
                    ..PersistentVolumeClaimVolumeSource::default()
                }),
                ..Volume::default()
            },
        });
    }

    if let Some(git_config) = app.spec.git.clone() {
        let volume = Volume {
            name: String::from(GIT_SYNC_DEST),
            empty_dir: Some(EmptyDirVolumeSource {
                ..EmptyDirVolumeSource::default()
            }),
            ..Volume::default()
        };
        pod_volumes.push(volume);

        //container mount
        let vol_mount = VolumeMount {
            name: String::from(GIT_SYNC_DEST),
            mount_path: "/src".to_owned(),
            ..VolumeMount::default()
        };
        volume_mounts.push(vol_mount);

        let mut git = container.clone();
        //remove the last one and add from req.dest
        git.image = git_config.image;
        git.command = None;
        git.volume_mounts = Some(vec![VolumeMount {
            name: String::from(GIT_SYNC_DEST),
            mount_path: "/tmp/git".to_owned(),
            ..VolumeMount::default()
        }]);
        git.env = Some(vec![
            EnvVar {
                name: "GIT_SYNC_REPO".to_owned(),
                value: git_config.repository,
                value_from: None,
            },
            EnvVar {
                name: "GIT_SYNC_PERIOD".to_owned(),
                value: git_config.period,
                value_from: None,
            },
            EnvVar {
                name: "GIT_SYNC_BRANCH".to_owned(),
                value: git_config.branch,
                value_from: None,
            },
            EnvVar {
                name: "GIT_SYNC_DEST".to_owned(),
                value: Some(GIT_SYNC_DEST.to_owned()),
                value_from: None,
            },
            EnvVar {
                name: "GIT_SYNC_ROOT".to_owned(),
                value: Some(GIT_SYNC_ROOT.to_owned()),
                value_from: None,
            },
            EnvVar {
                name: "GIT_SYNC_USERNAME".to_owned(),
                value: git_config.username.to_owned(),
                value_from: None,
            },
            EnvVar {
                name: "GIT_SYNC_PASSWORD".to_owned(),
                value: git_config.token.to_owned(),
                value_from: None,
            },
        ]);
        git.name = GIT_SYNC_DEST.to_owned();
        containers.push(git)
    }

    container.volume_mounts = Some(volume_mounts);
    containers.push(container);

    let pod_spec = PodSpec {
        init_containers: Some(vec![init_container]),
        containers,
        volumes: Some(pod_volumes),
        enable_service_links: app.spec.deployment.enable_service_links,
        service_account: app.spec.deployment.service_account.clone(),
        node_selector: node_selector(&app.spec.deployment),
        ..PodSpec::default()
    };

    let pod_template_spec = PodTemplateSpec {
        metadata: Some(ObjectMeta {
            labels: Some(meta.labels.clone()),
            annotations: Some(BTreeMap::from([(
                CONFIG_DIGEST_ANNOTATION.to_owned(),
                config_digest(app),
            )])),
            ..ObjectMeta::default()
        }),
        spec: Some(pod_spec),
    };

    let deployment_spec = DeploymentSpec {
        replicas: Some(deployment.container.replicas),
        selector: LabelSelector {
            match_labels: Some(meta.selector()),
            ..LabelSelector::default()
        },
        template: pod_template_spec,
        ..DeploymentSpec::default()
    };

    Deployment {
        metadata: ObjectMeta {
            name: Some(meta.name.to_owned()),
            annotations: Some(deployment.annotations),
            namespace: Some(meta.namespace.to_owned()),
            labels: Some(meta.labels.to_owned()),
            owner_references: Some(vec![meta.oref.to_owned()]),
            ..ObjectMeta::default()
        },
        spec: Some(deployment_spec),
        ..Deployment::default()
    }
}
