use k8s_openapi::api::{
    apps::v1::DeploymentSpec,
    core::v1::{
        ConfigMapEnvSource, ConfigMapVolumeSource, Container, ContainerPort, EmptyDirVolumeSource,
        EnvFromSource, EnvVar, PersistentVolumeClaimVolumeSource, PodSpec, PodTemplateSpec,
        SecretEnvSource, SecurityContext, Volume, VolumeMount,
    },
};

use crate::prelude::*;

const GIT_SYNC_DEST: &str = "git-sync";
const GIT_SYNC_ROOT: &str = "/tmp/git";

fn new(meta: &TappMeta, app: &TailoredApp, volumes: &BTreeMap<String, String>) -> Deployment {
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
        command,
        ports: Some(vec![ContainerPort {
            container_port: deployment.container.port,
            ..ContainerPort::default()
        }]),
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

    for (name, mount_path) in volumes.iter() {
        // Create a volume mount for each volume
        let vol_mount = VolumeMount {
            name: name.clone(),
            mount_path: mount_path.clone(),
            ..VolumeMount::default()
        };
        volume_mounts.push(vol_mount);

        if name.starts_with("files") {
            let volume = Volume {
                name: name.clone(),
                config_map: Some(ConfigMapVolumeSource {
                    name: Some(name.clone()),
                    ..ConfigMapVolumeSource::default()
                }),
                ..Volume::default()
            };
            pod_volumes.push(volume);
        } else if name.starts_with("pvc") {
            let volume = Volume {
                name: name.clone(),
                persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                    claim_name: name.clone(),
                    ..PersistentVolumeClaimVolumeSource::default()
                }),
                ..Volume::default()
            };
            pod_volumes.push(volume);
        }
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
        ..PodSpec::default()
    };

    let pod_template_spec = PodTemplateSpec {
        metadata: Some(ObjectMeta {
            labels: Some(meta.labels.clone()),
            ..ObjectMeta::default()
        }),
        spec: Some(pod_spec),
    };

    let deployment_spec = DeploymentSpec {
        replicas: Some(deployment.container.replicas),
        selector: LabelSelector {
            match_labels: Some(meta.labels.clone()),
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

pub async fn deploy(
    client: &Client,
    meta: &TappMeta,
    app: &TailoredApp,
    volumes: BTreeMap<String, String>,
) -> Result<Deployment, Error> {
    let deployment = new(meta, app, &volumes);
    let api: Api<Deployment> = Api::namespaced(client.clone(), &meta.namespace);
    match api.create(&PostParams::default(), &deployment).await {
        Ok(d) => Ok(d),
        Err(kube::Error::Api(e)) if e.code == 409 => update(client, meta, app, &volumes).await,
        Err(e) => {
            warn!(
                "Error while trying to update deployment {name}",
                name = meta.name
            );
            Err(Error::KubeError { source: e })
        },
    }
}

pub async fn update(
    client: &Client,
    meta: &TappMeta,
    app: &TailoredApp,
    volumes: &BTreeMap<String, String>,
) -> Result<Deployment, Error> {
    let mut deployment = new(meta, app, volumes);
    let api: Api<Deployment> = Api::namespaced(client.to_owned(), &meta.namespace);
    let resource_version = api.get(&meta.name).await?.metadata.resource_version;

    deployment.metadata.resource_version = resource_version;

    Ok(api
        .replace(&meta.name, &PostParams::default(), &deployment)
        .await?)
}
