use k8s_openapi::api::{
    apps::v1::DeploymentSpec,
    core::v1::{
        ConfigMapEnvSource, Container, ContainerPort, EnvFromSource, PodSpec, PodTemplateSpec,
        SecretEnvSource,
    },
};

use crate::prelude::*;

fn new(oref: &OwnerReference, namespace: &str, name: &str, app: &TailoredApp) -> Deployment {
    let deployment = app.spec.deployment.clone();
    let container = Container {
        name: name.to_owned(),
        image: Some(deployment.container.image.to_owned()),
        ports: Some(vec![ContainerPort {
            container_port: deployment.container.port,
            ..ContainerPort::default()
        }]),
        env_from: Some(vec![
            EnvFromSource {
                config_map_ref: Some(ConfigMapEnvSource {
                    name: Some(name.to_string()),
                    ..ConfigMapEnvSource::default()
                }),
                secret_ref: None,
                ..EnvFromSource::default()
            },
            EnvFromSource {
                config_map_ref: None,
                secret_ref: Some(SecretEnvSource {
                    name: Some(name.to_string()),
                    ..SecretEnvSource::default()
                }),
                ..EnvFromSource::default()
            },
        ]),
        ..Container::default()
    };
    let init_container = if let Some(build_cmd) = deployment.container.build_command {
            let mut init = container.clone();
            init.name = format!("{}-builder", name);
            init.command = Some(build_cmd.split_whitespace().map(String::from).collect());
            Some(vec![init])
    } else {
        None
    };
    let pod_spec = PodSpec {
        init_containers: init_container,
        containers: vec![container],
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
            name: Some(name.to_owned()),
            annotations: Some(deployment.annotations),
            namespace: Some(namespace.to_owned()),
            owner_references: Some(vec![oref.to_owned()]),
            ..ObjectMeta::default()
        },
        spec: Some(deployment_spec),
        ..Deployment::default()
    }
}

pub async fn deploy(
    client: Client,
    oref: &OwnerReference,
    namespace: &str,
    name: &str,
    app: &TailoredApp,
) -> Result<Deployment, Error> {
    let deployment = new(oref, namespace, name, app);
    let api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    match api.create(&PostParams::default(), &deployment).await {
        Ok(d) => Ok(d),
        Err(kube::Error::Api(e)) if e.code == 409 => {
            update(client, oref, namespace, name, app).await
        },
        Err(e) => {
            warn!("Error while trying to update deployment {name}");
            Err(Error::KubeError { source: e })
        },
    }
}

pub async fn update(
    client: Client,
    oref: &OwnerReference,
    namespace: &str,
    name: &str,
    app: &TailoredApp,
) -> Result<Deployment, Error> {
    let mut deployment = new(oref, namespace, name, &app.clone());
    let api: Api<Deployment> = Api::namespaced(client, namespace);
    let resource_version = api.get(name).await?.metadata.resource_version;

    deployment.metadata.resource_version = resource_version;

    Ok(api
        .replace(name, &PostParams::default(), &deployment)
        .await?)
}

pub async fn delete(client: Client, namespace: &str, name: &str) -> Result<(), Error> {
    let api: Api<Deployment> = Api::namespaced(client, namespace);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(e)) if e.code == 404 => {
            warn!("Deployment {name} already deleted");
            Ok(())
        },
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

pub async fn exists(client: Client, namespace: &str, name: &str) -> Result<bool, Error> {
    let api: Api<Deployment> = Api::namespaced(client, namespace);
    match api.get(name).await {
        Ok(_) => Ok(true),
        Err(kube::Error::Api(e)) if e.code == 404 => Ok(false),
        Err(e) => Err(Error::KubeError { source: e }),
    }
}
