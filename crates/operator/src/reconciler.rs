use crate::{
    actions::{apply_all, delete_all},
    finalizer, placement,
    prelude::*,
};

/// How often a settled app is re-checked for drift (a child deleted by hand, a node change).
/// Spec edits do not wait for this: the watch on TailoredApps triggers a reconcile at once.
const RESYNC: Duration = Duration::from_secs(60);
/// Requeue while a node-bound app has no placement yet: pods usually schedule within seconds.
const PLACEMENT_POLL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct TappMeta {
    pub name: String,
    pub namespace: String,
    pub labels: BTreeMap<String, String>,
    pub oref: OwnerReference,
}

impl TappMeta {
    /// Metadata for a child resource of this app with its own name (a PVC, a files ConfigMap):
    /// same namespace, labels and owner.
    pub fn child(&self, name: String) -> TappMeta {
        TappMeta {
            name,
            namespace: self.namespace.clone(),
            labels: self.labels.clone(),
            oref: self.oref.clone(),
        }
    }

    /// `k=v,k=v` selector matching every resource this app owns (the labels include `tapp=<name>`).
    pub fn label_selector(&self) -> String {
        self.labels
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Level-triggered: every reconcile makes the cluster match the spec. Server-side apply makes an
/// unchanged spec a no-op, so this is safe to run on each watch event and on the resync timer.
pub async fn reconcile(app: Arc<TailoredApp>, ctx: Arc<ContextData>) -> Result<Action, Error> {
    let client: Client = ctx.client.clone();

    let namespace: String = match app.namespace() {
        None => {
            return Err(Error::UserInputError(
                "Expected TailoredApp resource to be namespaced. Can't deploy to an unknown namespace."
                    .to_owned(),
            ));
        }
        Some(namespace) => namespace,
    };

    let oref = app
        .controller_owner_ref(&())
        .ok_or(Error::MissingObjectKey("metadata.uid"))?;
    let mut labels = app.spec.labels.clone();
    labels.insert("tapp".to_string(), app.name_any());
    let meta = TappMeta {
        name: app.name_any(),
        namespace: namespace.to_string(),
        labels,
        oref,
    };

    if app.meta().deletion_timestamp.is_some() {
        return delete_all(&client, &meta).await;
    }

    if !finalizer::present(&app) {
        finalizer::add(&client, &app).await?;
    }

    if let Err(e) = apply_all(&client, &meta, &app).await {
        // Best effort: surface the reason on the object itself before failing the reconcile.
        placement::report_error(&client, &meta, &e).await;
        return Err(e);
    }
    placement::publish(&client, &meta, &app).await?;

    let placed = app.status.as_ref().is_some_and(|s| !s.nodes.is_empty());
    let delay = if app.spec.is_node_bound() && !placed {
        PLACEMENT_POLL
    } else {
        RESYNC
    };
    Ok(Action::requeue(delay))
}
