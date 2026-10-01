use serde_json::{json, Value};

use crate::prelude::*;

pub static KUBETAILOR_FINALIZER: &str = "tailoredapps.kubetailor.io";

pub fn present(app: &TailoredApp) -> bool {
    app.finalizers().iter().any(|f| f == KUBETAILOR_FINALIZER)
}

/// Adds our finalizer, keeping any others already on the object (a merge patch replaces the
/// whole list, so it is rebuilt from the current one).
pub async fn add(client: &Client, app: &TailoredApp) -> Result<TailoredApp, Error> {
    let namespace = app
        .namespace()
        .ok_or(Error::MissingObjectKey("metadata.namespace"))?;
    let mut finalizers = app.finalizers().to_vec();
    finalizers.push(KUBETAILOR_FINALIZER.to_owned());
    patch_finalizers(client, &namespace, &app.name_any(), Some(finalizers)).await
}

/// Removes our finalizer only; `null` when nothing else is left so the field disappears.
pub async fn delete(client: &Client, namespace: &str, name: &str) -> Result<TailoredApp, Error> {
    let api: Api<TailoredApp> = Api::namespaced(client.to_owned(), namespace);
    let current = api.get(name).await?;
    let remaining: Vec<String> = current
        .finalizers()
        .iter()
        .filter(|f| *f != KUBETAILOR_FINALIZER)
        .cloned()
        .collect();
    patch_finalizers(
        client,
        namespace,
        name,
        (!remaining.is_empty()).then_some(remaining),
    )
    .await
}

async fn patch_finalizers(
    client: &Client,
    namespace: &str,
    name: &str,
    finalizers: Option<Vec<String>>,
) -> Result<TailoredApp, Error> {
    let api: Api<TailoredApp> = Api::namespaced(client.to_owned(), namespace);
    let patch: Value = json!({ "metadata": { "finalizers": finalizers } });
    Ok(api
        .patch(name, &PatchParams::default(), &Patch::Merge(&patch))
        .await?)
}
