use k8s_openapi::api::{
    apps::v1::DeploymentSpec,
    core::v1::{
        ConfigMapEnvSource, ConfigMapVolumeSource, Container, ContainerPort, EnvFromSource,
        PersistentVolumeClaimVolumeSource, PodSpec, PodTemplateSpec, SecretEnvSource,
        SecurityContext, Volume, VolumeMount,
    },
};

use crate::prelude::*;

fn new(meta: &TappMeta, app: &TailoredApp, volumes: &BTreeMap<String, String>) -> Deployment {
    let deployment = app.spec.deployment.clone();
    let mut container = Container {
        name: meta.name.to_owned(),
        image: Some(deployment.container.image.to_owned()),
        command: deployment.container.run_command,
        ports: Some(vec![ContainerPort {
            container_port: deployment.container.port,
            ..ContainerPort::default()
        }]),

        env_from: Some(vec![
            EnvFromSource {
                config_map_ref: Some(ConfigMapEnvSource {
                    name: Some(meta.name.to_owned()),
                    ..ConfigMapEnvSource::default()
                }),
                secret_ref: None,
                ..EnvFromSource::default()
            },
            EnvFromSource {
                config_map_ref: None,
                secret_ref: Some(SecretEnvSource {
                    name: Some(meta.name.to_owned()),
                    ..SecretEnvSource::default()
                }),
                ..EnvFromSource::default()
            },
        ]),
        security_context: Some(SecurityContext {
            allow_privilege_escalation: app.spec.deployment.allow_privilege_escalation,
            ..SecurityContext::default()
        }),
        ..Container::default()
    };

    // Vectors to hold volumes and volume mounts
    let mut volume_mounts = Vec::new();
    let mut pod_volumes = Vec::new();
    // Iterate through the volumes map
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

    container.volume_mounts = Some(volume_mounts);
    let mut init_containers = Vec::new();
    /*
    if let Some(git_repository) = deployment.container.git_repository{
        let mut git= container.clone();
        git.name = format!("{}-gitsync", meta.name);
        let clone_command: Vec<&str> = vec!["git", "clone", &git_repository];
        cloner.command = Some(clone_command.iter().map(|a| a.to_string()).collect());
        init_containers.push(cloner)
    };*/

    if let Some(build_cmd) = deployment.container.build_command {
        let mut builder = container.clone();
        builder.name = format!("{}-builder", meta.name);
        builder.command = Some(build_cmd);
        init_containers.push(builder)
    };
    let pod_spec = PodSpec {
        init_containers: Some(init_containers),
        containers: vec![container],
        volumes: Some(pod_volumes),
        enable_service_links: app.spec.deployment.enable_service_links,
        service_account: Some("limited".to_owned()),
        ..PodSpec::default()
    };

    let pod_template_spec = PodTemplateSpec {
        metadata: Some(ObjectMeta {
            labels: Some(app.spec.labels.clone()),
            ..ObjectMeta::default()
        }),
        spec: Some(pod_spec),
    };

    let deployment_spec = DeploymentSpec {
        replicas: Some(deployment.container.replicas),
        selector: LabelSelector {
            match_labels: Some(app.spec.labels.clone()),
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
