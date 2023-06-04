use serde::de::DeserializeOwned;
use tokio::time::Duration;

use crate::{configmap, deployment, finalizer, ingress, netpol, prelude::*, pvc, secret, service};

/// Action to be taken upon an `TailoredApp` resource during reconciliation
pub enum TailoredAppAction {
    /// Create the subresources
    Create,
    /// Delete all subresources created in the `Create` phase
    Delete,
    /// This `TailoredApp` resource is in desired state and requires no actions to be taken
    NoOp,
}

pub async fn deploy_all(client: &Client, meta: &TappMeta, app: &TailoredApp) -> Result<(), Error> {
    let name = app.name_any();

    // Deploy env_vars ConfigMap
    if let Some(env_vars) = app.spec.env_vars.as_ref() {
        configmap::deploy(client, meta, env_vars).await?;
    }

    // Deploy Secret
    if let Some(secrets) = app.spec.secrets.as_ref() {
        secret::deploy(client, meta, secrets).await?;
    }

    // Deploy Network policies
    if let Some(enable_netpol) = app.spec.deployment.deploy_network_policies {
        if enable_netpol {
            netpol::deploy(client, meta, app).await?;
        }
    }

    // Deploy Persistent Volumes
    let mut vol_mounts = BTreeMap::new();
    if let Some(volumes) = app.spec.deployment.container.volumes.clone() {
        for (i, (path, storage)) in volumes.iter().enumerate() {
            let pvc_name = format!("pvc-{}-{}", &name, i);
            let new_meta = TappMeta {
                name: pvc_name,
                labels: meta.labels.clone(),
                namespace: meta.namespace.to_string(),
                oref: meta.oref.clone(),
            };
            pvc::deploy(client, &new_meta, storage.clone()).await?;
            vol_mounts.insert(new_meta.name.to_owned(), path.to_string());
        }
    }

    // Deploy Files
    if let Some(files) = app.spec.deployment.container.files.clone() {
        // Group the paths by their parent directories
        let mut groups: HashMap<String, Vec<(&String, &String)>> = HashMap::new();
        for (path, data) in files.iter() {
            let path_buf = std::path::PathBuf::from(path);
            let parent_dir = match path_buf.parent() {
                Some(dir) => dir.to_string_lossy().into_owned(),
                None => {
                    return Err(Error::UserInputError(
                        "Invalid path: no parent directory".into(),
                    ));
                },
            };
            groups
                .entry(parent_dir)
                .or_insert_with(Vec::new)
                .push((path, data));
        }

        // Create a configMap for each path group
        for (i, (parent_dir, files)) in groups.into_iter().enumerate() {
            let cm_name = format!("files-{}-{}", name, i);
            let mut cm = BTreeMap::new();
            vol_mounts.insert(cm_name.to_owned(), parent_dir.clone());
            for (path, data) in files {
                let path_buf = std::path::PathBuf::from(path);
                let file_name = match path_buf.file_name() {
                    Some(fname) => fname.to_string_lossy().into_owned(),
                    None => return Err(Error::UserInputError("Invalid path: no file name".into())),
                };
                cm.insert(file_name.to_string(), data.to_string());
            }
            let new_meta = TappMeta {
                name: cm_name,
                labels: meta.labels.clone(),
                namespace: meta.namespace.to_string(),
                oref: meta.oref.clone(),
            };
            configmap::deploy(client, &new_meta, &cm).await?;
        }
    }

    // Deploy Deployment
    deployment::deploy(client, meta, app, vol_mounts).await?;

    // Deploy Service
    //service::deploy(client.clone(), namespace, &name, app).await?;
    service::deploy(client, meta, app).await?;

    // Deploy Ingress
    //ingress::deploy(client.clone(), namespace, &name, app).await?;
    ingress::deploy(client, meta, app).await?;

    Ok(())
}

pub async fn delete_all(client: &Client, meta: &TappMeta) -> Result<Action, Error> {
    delete::<ConfigMap>(client, meta).await?;
    delete::<Deployment>(client, meta).await?;
    delete::<Secret>(client, meta).await?;
    delete::<Ingress>(client, meta).await?;
    delete::<Service>(client, meta).await?;
    delete::<PersistentVolumeClaim>(client, meta).await?;

    if exists::<PersistentVolumeClaim>(client, meta).await? {
        Ok(Action::requeue(Duration::from_secs(10)))
    } else {
        finalizer::delete(client, &meta.namespace, &meta.name).await?;
        Ok(Action::await_change())
    }
}

pub async fn delete<T>(client: &Client, meta: &TappMeta) -> Result<(), Error>
where
    T: Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + DeserializeOwned
        + std::fmt::Debug
        + Clone,
    <T as Resource>::Scope: ResourceScope,
{
    let api: Api<T> = Api::namespaced(client.to_owned(), &meta.namespace);
    let dp = DeleteParams::default();
    match api.delete(&meta.name, &dp).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(e)) if e.code == 404 => {
            warn!("Resource {meta:?} already deleted");
            Ok(())
        },
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

pub async fn exists<T>(client: &Client, meta: &TappMeta) -> Result<bool, Error>
where
    T: Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + DeserializeOwned
        + std::fmt::Debug
        + Clone,
{
    let api: Api<T> = Api::namespaced(client.to_owned(), &meta.namespace);
    match api.get(&meta.name).await {
        Ok(_) => Ok(true),
        Err(kube::Error::Api(e)) if e.code == 404 => Ok(false),
        Err(e) => Err(Error::KubeError { source: e }),
    }
}
