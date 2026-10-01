use crate::prelude::*;

pub fn new(meta: &TappMeta, data: BTreeMap<String, String>) -> ConfigMap {
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
