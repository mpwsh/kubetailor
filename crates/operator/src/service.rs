use k8s_openapi::api::core::v1::{ServicePort, ServiceSpec};

use crate::prelude::*;

fn new(meta: &TappMeta, app: &TailoredApp) -> Service {
    let app = app.spec.clone();
    let service_port = ServicePort {
        name: Some(meta.name.to_owned()),
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
            name: Some(meta.name.to_owned()),
            namespace: Some(meta.namespace.to_owned()),
            ..ObjectMeta::default()
        },
        spec: Some(service_spec),
        ..Service::default()
    }
}

pub async fn deploy(client: &Client, meta: &TappMeta, app: &TailoredApp) -> Result<Service, Error> {
    let service = new(meta, app);
    let api: Api<Service> = Api::namespaced(client.clone(), &meta.namespace);
    match api.create(&PostParams::default(), &service).await {
        Ok(s) => Ok(s),
        Err(kube::Error::Api(e)) if e.code == 409 => update(client, meta, app).await,
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

pub async fn update(client: &Client, meta: &TappMeta, app: &TailoredApp) -> Result<Service, Error> {
    let mut service = new(meta, app);
    let api: Api<Service> = Api::namespaced(client.to_owned(), &meta.namespace);

    let resource_version = api.get(&meta.name).await?.metadata.resource_version;
    service.metadata.resource_version = resource_version;

    Ok(api
        .replace(&meta.name, &PostParams::default(), &service)
        .await?)
}
