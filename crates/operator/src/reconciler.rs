use crate::{
    actions::{delete_all, deploy_all, TailoredAppAction},
    finalizer,
    prelude::*,
};

#[derive(Debug)]
pub struct TappMeta {
    pub name: String,
    pub namespace: String,
    pub labels: BTreeMap<String, String>,
    pub oref: OwnerReference,
}

pub async fn reconcile(app: Arc<TailoredApp>, ctx: Arc<ContextData>) -> Result<Action, Error> {
    let client: Client = ctx.client.clone();

    let namespace: String = match app.namespace() {
        None => {
            return Err(Error::UserInputError(
                "Expected TailoredApp resource to be namespaced. Can't deploy to an unknown namespace."
                    .to_owned(),
            ));
        },
        Some(namespace) => namespace,
    };

    let oref = app.controller_owner_ref(&()).unwrap();

    let meta = TappMeta {
        name: app.name_any(),
        namespace: namespace.to_string(),
        labels: app.spec.labels.clone(),
        oref: oref.clone(),
    };
    match determine_action(&app) {
        TailoredAppAction::Create => {
            // Apply the finalizer first. If that fails, the `?` operator invokes automatic conversion
            // of `kube::Error` to the `Error` defined in this crate.
            finalizer::add(&client, &namespace, &app.name_any()).await?;

            // Invoke creation of all the resources
            deploy_all(&client, &meta, &app).await?;

            Ok(Action::requeue(Duration::from_secs(10)))
        },
        TailoredAppAction::Delete => delete_all(&client, &meta).await,
        // The resource is already in desired state, do nothing and re-check after 10 seconds
        TailoredAppAction::NoOp => Ok(Action::requeue(Duration::from_secs(10))),
    }
}

/// Resources arrives into reconciliation queue in a certain state. This function looks at
/// the state of given `TailoredApp` resource and decides which actions needs to be performed.
/// The finite set of possible actions is represented by the `Action` enum.
///
/// # Arguments
/// - `app`: A reference to `TailoredApp` being reconciled to decide next action upon.
fn determine_action(app: &TailoredApp) -> TailoredAppAction {
    if app.meta().deletion_timestamp.is_some() {
        TailoredAppAction::Delete
    } else if app
        .meta()
        .finalizers
        .as_ref()
        .map_or(true, |finalizers| finalizers.is_empty())
    {
        TailoredAppAction::Create
    } else {
        TailoredAppAction::NoOp
    }
}
