use kubetailor::k8s_openapi::{
    api::core::v1::{PersistentVolumeClaim, PersistentVolumeClaimSpec, ResourceRequirements},
    apimachinery::pkg::api::resource::Quantity,
};

use crate::prelude::*;

pub fn new(meta: &TappMeta, storage: &str) -> PersistentVolumeClaim {
    PersistentVolumeClaim {
        metadata: ObjectMeta {
            name: Some(meta.name.to_owned()),
            namespace: Some(meta.namespace.to_owned()),
            labels: Some(meta.labels.to_owned()),
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
