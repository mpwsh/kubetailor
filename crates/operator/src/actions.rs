use tokio::time::Duration;

use crate::{configmap, deployment, finalizer, ingress, prelude::*, secret, service};

/// Action to be taken upon an `TailoredApp` resource during reconciliation
pub enum TailoredAppAction {
    /// Create the subresources
    Create,
    /// Delete all subresources created in the `Create` phase
    Delete,
    /// This `TailoredApp` resource is in desired state and requires no actions to be taken
    NoOp,
}

pub async fn deploy_all(
    client: Client,
    oref: OwnerReference,
    namespace: &str,
    app: &TailoredApp,
) -> Result<(), Error> {
    let name = app.name_any();

    // Deploy ConfigMap
    configmap::deploy(
        client.clone(),
        &oref,
        namespace,
        &name,
        app.spec.env_vars.clone(),
    )
    .await?;

    // Deploy Secret
    secret::deploy(client.clone(), namespace, &name, app.spec.secrets.clone()).await?;

    // Deploy Deployment
    deployment::deploy(client.clone(), &oref, namespace, &name, app).await?;

    // Deploy Service
    service::deploy(client.clone(), namespace, &name, app).await?;

    // Deploy Ingress
    ingress::deploy(client, namespace, &name, app).await?;

    Ok(())
}

pub async fn delete_all(client: Client, namespace: &str, name: &str) -> Result<Action, Error> {
    // Delete Deployment
    deployment::delete(client.clone(), namespace, name).await?;

    // Delete ConfigMap
    configmap::delete(client.clone(), namespace, name).await?;

    // Delete Secret
    secret::delete(client.clone(), namespace, name).await?;

    // Delete Ingress
    ingress::delete(client.clone(), namespace, name).await?;

    // Delete Service
    service::delete(client.clone(), namespace, name).await?;

    // Check if all resources are deleted
    if deployment::exists(client.clone(), namespace, name).await?
        || configmap::exists(client.clone(), namespace, name).await?
        || secret::exists(client.clone(), namespace, name).await?
        || ingress::exists(client.clone(), namespace, name).await?
        || service::exists(client.clone(), namespace, name).await?
    {
        // If some resources still exist, requeue with a delay
        Ok(Action::requeue(Duration::from_secs(10)))
    } else {
        // If all resources are deleted, remove the finalizer
        finalizer::delete(client, namespace, name).await?;
        Ok(Action::await_change())
    }
}
