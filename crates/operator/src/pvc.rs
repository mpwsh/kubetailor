use k8s_openapi::{
    api::core::v1::{PersistentVolumeClaim, PersistentVolumeClaimSpec, ResourceRequirements},
    apimachinery::pkg::api::resource::Quantity,
};

use crate::prelude::*;

fn new(meta: &TappMeta, storage: &str) -> PersistentVolumeClaim {
    PersistentVolumeClaim {
        metadata: ObjectMeta {
            name: Some(meta.name.to_owned()),
            namespace: Some(meta.namespace.to_owned()),
            owner_references: Some(vec![meta.oref.to_owned()]),
            ..ObjectMeta::default()
        },
        spec: Some(PersistentVolumeClaimSpec {
            access_modes: Some(vec!["ReadWriteOnce".to_string()]),
            resources: Some(ResourceRequirements {
                requests: {
                    let mut map = BTreeMap::new();
                    map.insert("storage".to_string(), Quantity(storage.to_string()));
                    Some(map)
                },
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub async fn deploy(
    client: &Client,
    meta: &TappMeta,
    storage: String,
) -> Result<PersistentVolumeClaim, Error> {
    let pvc = new(meta, &storage);
    let api: Api<PersistentVolumeClaim> = Api::namespaced(client.to_owned(), &meta.namespace);
    match api.create(&PostParams::default(), &pvc).await {
        Ok(pvc) => Ok(pvc),
        Err(kube::Error::Api(e)) if e.code == 409 => {
            update(client, meta, &storage).await
        },
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

pub async fn update(
    client: &Client,
    meta: &TappMeta,
    storage: &str,
) -> Result<PersistentVolumeClaim, Error> {
    let mut pvc = new(meta, storage);
    let api: Api<PersistentVolumeClaim> = Api::namespaced(client.to_owned(), &meta.namespace);

    let resource_version = api.get(&meta.name).await?.metadata.resource_version;
    pvc.metadata.resource_version = resource_version;

    Ok(api.replace(&meta.name, &PostParams::default(), &pvc).await?)
}
