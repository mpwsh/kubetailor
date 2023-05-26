use crate::prelude::*;

fn new(
    oref: &OwnerReference,
    namespace: &str,
    name: &str,
    data: BTreeMap<String, String>,
) -> ConfigMap {
    ConfigMap {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(namespace.to_owned()),
            owner_references: Some(vec![oref.to_owned()]),
            ..ObjectMeta::default()
        },
        data: Some(data),
        ..ConfigMap::default()
    }
}

pub async fn deploy(
    client: Client,
    oref: &OwnerReference,
    namespace: &str,
    name: &str,
    data: BTreeMap<String, String>,
) -> Result<ConfigMap, Error> {
    let cm = new(oref, namespace, name, data.clone());
    let api: Api<ConfigMap> = Api::namespaced(client.clone(), namespace);
    match api.create(&PostParams::default(), &cm).await {
        Ok(cm) => Ok(cm),
        Err(kube::Error::Api(e)) if e.code == 409 => {
            update(client, oref, namespace, name, data).await
        },
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

pub async fn update(
    client: Client,
    oref: &OwnerReference,
    namespace: &str,
    name: &str,
    data: BTreeMap<String, String>,
) -> Result<ConfigMap, Error> {
    let mut cm = new(oref, namespace, name, data);
    let api: Api<ConfigMap> = Api::namespaced(client, namespace);

    let resource_version = api.get(name).await?.metadata.resource_version;
    cm.metadata.resource_version = resource_version;

    Ok(api.replace(name, &PostParams::default(), &cm).await?)
}

pub async fn delete(client: Client, namespace: &str, name: &str) -> Result<(), Error> {
    let api: Api<ConfigMap> = Api::namespaced(client, namespace);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(e)) if e.code == 404 => {
            warn!("ConfigMap {name} already deleted");
            Ok(())
        },
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

pub async fn exists(client: Client, namespace: &str, name: &str) -> Result<bool, Error> {
    let api: Api<ConfigMap> = Api::namespaced(client, namespace);
    match api.get(name).await {
        Ok(_) => Ok(true),
        Err(kube::Error::Api(e)) if e.code == 404 => Ok(false),
        Err(e) => Err(Error::KubeError { source: e }),
    }
}
