use base64::{engine::general_purpose, Engine as _};

use crate::prelude::*;

fn new(namespace: &str, name: &str, data: BTreeMap<String, String>) -> Secret {
    let encoded_data: BTreeMap<String, ByteString> = data
        .into_iter()
        .map(|(k, v)| {
            (
                k,
                ByteString(general_purpose::STANDARD.encode(v).into_bytes()),
            )
        })
        .collect();

    Secret {
        metadata: ObjectMeta {
            name: Some(name.to_owned()),
            namespace: Some(namespace.to_owned()),
            ..ObjectMeta::default()
        },
        data: Some(encoded_data),
        ..Secret::default()
    }
}

pub async fn deploy(
    client: Client,
    namespace: &str,
    name: &str,
    data: BTreeMap<String, String>,
) -> Result<Secret, Error> {
    let secret = new(namespace, name, data.clone());
    let api: Api<Secret> = Api::namespaced(client.clone(), namespace);
    match api.create(&PostParams::default(), &secret).await {
        Ok(secret) => Ok(secret),
        Err(kube::Error::Api(e)) if e.code == 409 => update(client, namespace, name, data).await,
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

pub async fn update(
    client: Client,
    namespace: &str,
    name: &str,
    data: BTreeMap<String, String>,
) -> Result<Secret, Error> {
    let mut secret = new(namespace, name, data);
    let api: Api<Secret> = Api::namespaced(client, namespace);

    let resource_version = api.get(name).await?.metadata.resource_version;
    secret.metadata.resource_version = resource_version;

    Ok(api.replace(name, &PostParams::default(), &secret).await?)
}

pub async fn delete(client: Client, namespace: &str, name: &str) -> Result<(), Error> {
    let api: Api<Secret> = Api::namespaced(client, namespace);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(e)) if e.code == 404 => {
            warn!("Secret {name} already deleted");
            Ok(())
        },
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

pub async fn exists(client: Client, namespace: &str, name: &str) -> Result<bool, Error> {
    let api: Api<Secret> = Api::namespaced(client, namespace);
    match api.get(name).await {
        Ok(_) => Ok(true),
        Err(kube::Error::Api(e)) if e.code == 404 => Ok(false),
        Err(e) => Err(Error::KubeError { source: e }),
    }
}
