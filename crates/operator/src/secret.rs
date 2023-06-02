use crate::prelude::*;

fn new(meta: &TappMeta, data: BTreeMap<String, String>) -> Secret {
    let encoded_data: BTreeMap<String, ByteString> = data
        .into_iter()
        .map(|(k, v)| {
            (
                k,
                ByteString(v.into_bytes()),
            )
        })
        .collect();

    Secret {
        metadata: ObjectMeta {
            name: Some(meta.name.to_owned()),
            namespace: Some(meta.namespace.to_owned()),
            labels: Some(meta.labels.to_owned()),
            ..ObjectMeta::default()
        },
        data: Some(encoded_data),
        ..Secret::default()
    }
}

pub async fn deploy(
    client: &Client,
    meta: &TappMeta,
    data: &BTreeMap<String, String>,
) -> Result<Secret, Error> {
    let secret = new(meta, data.clone());
    let api: Api<Secret> = Api::namespaced(client.to_owned(), &meta.namespace);
    match api.create(&PostParams::default(), &secret).await {
        Ok(secret) => Ok(secret),
        Err(kube::Error::Api(e)) if e.code == 409 => update(client, meta, data).await,
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

pub async fn update(
    client: &Client,
    meta: &TappMeta,
    data: &BTreeMap<String, String>,
) -> Result<Secret, Error> {
    let mut secret = new(meta, data.clone());
    let api: Api<Secret> = Api::namespaced(client.to_owned(), &meta.namespace);

    let resource_version = api.get(&meta.name).await?.metadata.resource_version;
    secret.metadata.resource_version = resource_version;

    Ok(api
        .replace(&meta.name, &PostParams::default(), &secret)
        .await?)
}
