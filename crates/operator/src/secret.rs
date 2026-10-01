use crate::prelude::*;

pub fn new(meta: &TappMeta, data: BTreeMap<String, String>) -> Secret {
    let encoded_data: BTreeMap<String, ByteString> = data
        .into_iter()
        .map(|(k, v)| (k, ByteString(v.into_bytes())))
        .collect();

    Secret {
        metadata: ObjectMeta {
            name: Some(meta.name.to_owned()),
            namespace: Some(meta.namespace.to_owned()),
            owner_references: Some(vec![meta.oref.to_owned()]),
            labels: Some(meta.labels.to_owned()),
            ..ObjectMeta::default()
        },
        data: Some(encoded_data),
        ..Secret::default()
    }
}
