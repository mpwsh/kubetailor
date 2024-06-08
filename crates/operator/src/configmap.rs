use crate::prelude::*;

fn new(meta: &TappMeta, data: BTreeMap<String, String>) -> ConfigMap {
    ConfigMap {
        metadata: ObjectMeta {
            name: Some(meta.name.to_owned()),
            namespace: Some(meta.namespace.to_owned()),
            owner_references: Some(vec![meta.oref.to_owned()]),
            labels: Some(meta.labels.to_owned()),
            ..ObjectMeta::default()
        },
        data: Some(data),
        ..ConfigMap::default()
    }
}

pub async fn deploy(
    client: &Client,
    meta: &TappMeta,
    data: &BTreeMap<String, String>,
) -> Result<ConfigMap, Error> {
    let cm = new(meta, data.clone());
    let api: Api<ConfigMap> = Api::namespaced(client.clone(), &meta.namespace);
    match api.create(&PostParams::default(), &cm).await {
        Ok(cm) => Ok(cm),
        Err(kubetailor::kube::Error::Api(e)) if e.code == 409 => update(client, meta, data).await,
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

pub async fn update(
    client: &Client,
    meta: &TappMeta,
    data: &BTreeMap<String, String>,
) -> Result<ConfigMap, Error> {
    let mut cm = new(meta, data.clone());
    let api: Api<ConfigMap> = Api::namespaced(client.to_owned(), &meta.namespace);

    let resource_version = api.get(&meta.name).await?.metadata.resource_version;
    cm.metadata.resource_version = resource_version;

    Ok(api.replace(&meta.name, &PostParams::default(), &cm).await?)
}
