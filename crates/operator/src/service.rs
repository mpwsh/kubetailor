use k8s_openapi::api::core::v1::{ServicePort, ServiceSpec};

use crate::prelude::*;

fn new(namespace: &str, name: &str, app: &TailoredApp) -> Service {
    let app = app.spec.clone();
    let service_port = ServicePort {
        name: Some(name.to_string()),
        port: app.deployment.container.port,
        target_port: Some(IntOrString::Int(app.deployment.container.port)),
        ..ServicePort::default()
    };

    let service_spec = ServiceSpec {
        selector: Some(app.labels),
        ports: Some(vec![service_port]),
        ..ServiceSpec::default()
    };

    Service {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(namespace.to_owned()),
            ..ObjectMeta::default()
        },
        spec: Some(service_spec),
        ..Service::default()
    }
}

pub async fn deploy(
    client: Client,
    namespace: &str,
    name: &str,
    app: &TailoredApp,
) -> Result<Service, Error> {
    let service = new(namespace, name, &app.clone());
    let api: Api<Service> = Api::namespaced(client.clone(), namespace);
    match api.create(&PostParams::default(), &service).await {
        Ok(s) => Ok(s),
        Err(kube::Error::Api(e)) if e.code == 409 => update(client, namespace, name, app).await,
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

pub async fn update(
    client: Client,
    namespace: &str,
    name: &str,
    app: &TailoredApp,
) -> Result<Service, Error> {
    let mut service = new(namespace, name, &app.clone());
    let api: Api<Service> = Api::namespaced(client, namespace);

    let resource_version = api.get(name).await?.metadata.resource_version;
    service.metadata.resource_version = resource_version;

    Ok(api.replace(name, &PostParams::default(), &service).await?)
}

pub async fn delete(client: Client, namespace: &str, name: &str) -> Result<(), Error> {
    let api: Api<Service> = Api::namespaced(client, namespace);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(e)) if e.code == 404 => {
            warn!("Service {name} already deleted");
            Ok(())
        },
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

pub async fn exists(client: Client, namespace: &str, name: &str) -> Result<bool, Error> {
    let api: Api<Service> = Api::namespaced(client, namespace);
    match api.get(name).await {
        Ok(_) => Ok(true),
        Err(kube::Error::Api(e)) if e.code == 404 => Ok(false),
        Err(e) => Err(Error::KubeError { source: e }),
    }
}
