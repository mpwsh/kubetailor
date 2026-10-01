use std::collections::BTreeSet;

use serde::de::DeserializeOwned;
use tokio::time::Duration;

use crate::{
    apply::{apply, prune},
    configmap,
    deployment::{self, Mount, MountSource},
    finalizer, ingress, netpol,
    prelude::*,
    pvc, secret, service,
};

/// Names of the resources the spec asks for, per kind, so anything else carrying the app's
/// labels can be pruned afterwards.
#[derive(Default)]
struct Desired {
    configmaps: BTreeSet<String>,
    secrets: BTreeSet<String>,
    services: BTreeSet<String>,
    ingresses: BTreeSet<String>,
    netpols: BTreeSet<String>,
}

/// Short, stable identifier for a path: child resources are named after *what* they mount, not
/// their position in a list, so adding a volume or a file directory never renames another one
/// (which would remount and restart the pod, and for PVCs lose data).
fn path_id(path: &str) -> String {
    // FNV-1a, 64-bit. Not security-relevant; only determinism matters.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:08x}", (hash >> 32) ^ (hash & 0xffff_ffff))
}

/// Makes the cluster match the spec: every child resource is server-side applied, then
/// resources the spec no longer asks for are removed. Safe to run on every reconcile.
pub async fn apply_all(client: &Client, meta: &TappMeta, app: &TailoredApp) -> Result<(), Error> {
    let name = &meta.name;
    let ns = &meta.namespace;
    let mut desired = Desired::default();

    // Env ConfigMap
    if let Some(env) = app.spec.env.as_ref() {
        apply(client, ns, &configmap::new(meta, env.clone())).await?;
        desired.configmaps.insert(name.clone());
    }

    // Secret
    if let Some(secrets) = app.spec.secrets.as_ref() {
        apply(client, ns, &secret::new(meta, secrets.clone())).await?;
        desired.secrets.insert(name.clone());
    }

    let mut mounts = Vec::new();

    // Persistent volumes. Never pruned on update: a volume dropped from the spec keeps its data
    // until the app itself is deleted.
    if let Some(volumes) = app.spec.deployment.container.volumes.as_ref() {
        for (path, storage) in volumes {
            let id = path_id(path);
            let pvc_meta = meta.child(format!("pvc-{name}-{id}"));
            apply(client, ns, &pvc::new(&pvc_meta, storage)).await?;
            mounts.push(Mount {
                volume: format!("pvc-{id}"),
                path: path.clone(),
                source: MountSource::Pvc(pvc_meta.name),
            });
        }
    }

    // Files: one ConfigMap per parent directory (a ConfigMap mounts as a directory).
    if let Some(files) = app.spec.deployment.container.files.as_ref() {
        let mut groups: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for (path, data) in files {
            let path_buf = std::path::PathBuf::from(path);
            let parent = path_buf
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .ok_or_else(|| {
                    Error::UserInputError(format!("file `{path}`: no parent directory"))
                })?;
            let file_name = path_buf
                .file_name()
                .ok_or_else(|| Error::UserInputError(format!("file `{path}`: no file name")))?;
            groups
                .entry(parent.to_string_lossy().into_owned())
                .or_default()
                .insert(file_name.to_string_lossy().into_owned(), data.clone());
        }
        for (dir, data) in groups {
            let id = path_id(&dir);
            let cm_meta = meta.child(format!("files-{name}-{id}"));
            apply(client, ns, &configmap::new(&cm_meta, data)).await?;
            desired.configmaps.insert(cm_meta.name.clone());
            mounts.push(Mount {
                volume: format!("files-{id}"),
                path: dir,
                source: MountSource::ConfigMap(cm_meta.name),
            });
        }
    }

    // Deployment
    apply(client, ns, &deployment::new(meta, app, &mounts)).await?;

    // Services
    for svc in service::all(meta, app) {
        if let Some(svc_name) = svc.metadata.name.clone() {
            desired.services.insert(svc_name);
        }
        apply(client, ns, &svc).await?;
    }

    // Ingress (needs domains and an HTTP port; domains alone only name the app for DNS)
    if app.spec.wants_ingress() {
        apply(client, ns, &ingress::new(meta, app)?).await?;
        desired.ingresses.insert(name.clone());
    } else if !app.spec.hostnames().is_empty() {
        info!(
            "{name}: has domains but no `container.port`; no Ingress, DNS will point at the node"
        );
    }

    // Network policies
    if app.spec.deployment.deploy_network_policies == Some(true) {
        apply(client, ns, &netpol::new(meta, app)).await?;
        desired.netpols.insert(name.clone());
    }

    // What the spec stopped asking for goes away.
    prune::<ConfigMap>(client, meta, &desired.configmaps).await?;
    prune::<Secret>(client, meta, &desired.secrets).await?;
    prune::<Service>(client, meta, &desired.services).await?;
    prune::<Ingress>(client, meta, &desired.ingresses).await?;
    prune::<NetworkPolicy>(client, meta, &desired.netpols).await?;

    Ok(())
}

pub async fn delete_all(client: &Client, meta: &TappMeta) -> Result<Action, Error> {
    delete::<ConfigMap>(client, meta).await?;
    delete::<Deployment>(client, meta).await?;
    delete::<Secret>(client, meta).await?;
    delete::<Ingress>(client, meta).await?;
    delete::<NetworkPolicy>(client, meta).await?;
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
    let lp = ListParams::default().labels(&meta.label_selector());
    let dp = DeleteParams::default();
    match api.delete_collection(&dp, &lp).await {
        Ok(_) => Ok(()),
        Err(kubetailor::kube::Error::Api(e)) if e.code == 404 => {
            warn!("Resource {meta:?} already deleted");
            Ok(())
        }
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
    let lp = ListParams::default().labels(&meta.label_selector());
    match api.list(&lp).await {
        Ok(resources) => Ok(!resources.items.is_empty()),
        Err(kubetailor::kube::Error::Api(e)) if e.code == 404 => Ok(false),
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_id_is_stable_and_distinct() {
        assert_eq!(path_id("/data"), path_id("/data"));
        assert_ne!(path_id("/data"), path_id("/data2"));
        assert_eq!(path_id("/usr/share/nginx/html").len(), 8);
    }
}
