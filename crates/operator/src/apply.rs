//! Server-side apply for the resources a TailoredApp owns.
//!
//! Every child resource is written with `PATCH ... application/apply-patch+yaml` under one field
//! manager. Compared to the create-then-replace dance this has three properties the reconciler
//! relies on:
//! - only the fields the operator sets are owned, so what other controllers write (the
//!   Deployment controller's revision annotation, allocated `clusterIP`s and node ports, a
//!   `kubectl rollout restart` stamp) survives;
//! - applying an unchanged object is a no-op on the API server: no new resourceVersion, no watch
//!   event, so applying on every reconcile cannot feed back into itself;
//! - a field the operator stops sending is removed, which is what makes spec edits "live".

use std::collections::BTreeSet;

use serde::de::DeserializeOwned;

use crate::prelude::*;

/// Field manager name recorded in `metadata.managedFields`.
pub const FIELD_MANAGER: &str = "kubetailor";

/// Applies `obj` (create or update), returning the server's view of it.
pub async fn apply<K>(client: &Client, namespace: &str, obj: &K) -> Result<K, Error>
where
    K: Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + Serialize
        + DeserializeOwned
        + Clone
        + Debug,
{
    let name = obj
        .meta()
        .name
        .as_deref()
        .ok_or(Error::MissingObjectKey("metadata.name"))?;
    let api: Api<K> = Api::namespaced(client.clone(), namespace);
    let params = PatchParams::apply(FIELD_MANAGER).force();
    Ok(api.patch(name, &params, &Patch::Apply(obj)).await?)
}

/// Deletes every `K` carrying the app's labels whose name is not in `keep`. This is how a spec
/// edit that drops a section (the `nodePort` Service, the Ingress, the env ConfigMap) takes its
/// resource with it.
pub async fn prune<K>(
    client: &Client,
    meta: &TappMeta,
    keep: &BTreeSet<String>,
) -> Result<(), Error>
where
    K: Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + DeserializeOwned
        + Clone
        + Debug,
{
    let api: Api<K> = Api::namespaced(client.clone(), &meta.namespace);
    let existing = api
        .list(&ListParams::default().labels(&meta.label_selector()))
        .await?;
    for item in existing.items {
        let Some(name) = item.meta().name.clone() else {
            continue;
        };
        if keep.contains(&name) || item.meta().deletion_timestamp.is_some() {
            continue;
        }
        info!("{}: pruning {} {name}", meta.name, K::kind(&()));
        match api.delete(&name, &DeleteParams::default()).await {
            Ok(_) => {}
            Err(kubetailor::kube::Error::Api(e)) if e.code == 404 => {}
            Err(e) => return Err(Error::KubeError { source: e }),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use kubetailor::crd::{Container, Deployment as TappDeployment, TailoredAppSpec};

    use super::*;
    use crate::{deployment, netpol, service};

    fn fixture() -> (TappMeta, TailoredApp) {
        let app = TailoredApp::new(
            "web",
            TailoredAppSpec {
                labels: BTreeMap::from([("owner".to_owned(), "x".to_owned())]),
                deployment: TappDeployment {
                    annotations: BTreeMap::new(),
                    enable_service_links: None,
                    service_account: None,
                    allow_privilege_escalation: None,
                    allow_root: None,
                    run_as_user: None,
                    run_as_group: None,
                    deploy_network_policies: Some(true),
                    region: None,
                    container: Container {
                        image: "nginx".into(),
                        port: Some(80),
                        replicas: 1,
                        ..Container::default()
                    },
                },
                ingress: None,
                env: None,
                secrets: None,
                git: None,
            },
        );
        let meta = TappMeta {
            name: "web".into(),
            namespace: "kubetailor".into(),
            labels: BTreeMap::from([("tapp".to_owned(), "web".to_owned())]),
            oref: OwnerReference::default(),
        };
        (meta, app)
    }

    /// A Deployment's selector cannot change, and the group label can: pods are selected by the
    /// app's identity alone, while the pod template keeps the full label set for the network
    /// policies. Services select the same way.
    #[test]
    fn pods_are_selected_by_identity_only() {
        let (mut meta, app) = fixture();
        meta.labels.insert("owner".to_owned(), "x".to_owned());
        meta.labels.insert("group".to_owned(), "games".to_owned());
        meta.labels
            .insert("fingerprint".to_owned(), "abc".to_owned());
        let d = deployment::new(&meta, &app, &[]);
        let spec = d.spec.unwrap();
        let only_tapp = BTreeMap::from([("tapp".to_owned(), "web".to_owned())]);
        assert_eq!(spec.selector.match_labels, Some(only_tapp.clone()));
        assert_eq!(
            spec.template.metadata.unwrap().labels,
            Some(meta.labels.clone())
        );
        for svc in service::all(&meta, &app) {
            assert_eq!(svc.spec.unwrap().selector, Some(only_tapp.clone()));
        }
    }

    /// Server-side apply rejects bodies without apiVersion/kind; make sure every builder's
    /// output carries them (k8s-openapi adds them on serialisation, but that is easy to lose by
    /// wrapping types).
    #[test]
    fn applied_objects_carry_type_meta() {
        let (meta, app) = fixture();
        let objects = vec![
            serde_json::to_value(deployment::new(&meta, &app, &[])).unwrap(),
            serde_json::to_value(&service::all(&meta, &app)[0]).unwrap(),
            serde_json::to_value(netpol::new(&meta, &app)).unwrap(),
            serde_json::to_value(&app).unwrap(),
        ];
        for o in objects {
            assert!(o["apiVersion"].is_string(), "{o}");
            assert!(o["kind"].is_string(), "{o}");
            assert!(o["metadata"]["name"].is_string(), "{o}");
            assert!(o["metadata"].get("resourceVersion").is_none(), "{o}");
        }
    }

    /// The merge keys server-side apply uses for list fields must be present, or two applies
    /// disagree about which element is which.
    #[test]
    fn list_merge_keys_are_set() {
        let (meta, app) = fixture();
        let d = serde_json::to_value(deployment::new(&meta, &app, &[])).unwrap();
        for c in d["spec"]["template"]["spec"]["containers"]
            .as_array()
            .unwrap()
        {
            assert!(c["name"].is_string());
            for p in c["ports"].as_array().unwrap_or(&vec![]) {
                assert!(
                    p["containerPort"].is_number() && p["protocol"].is_string(),
                    "{p}"
                );
            }
        }
        let s = serde_json::to_value(&service::all(&meta, &app)[0]).unwrap();
        for p in s["spec"]["ports"].as_array().unwrap() {
            assert!(p["port"].is_number() && p["protocol"].is_string(), "{p}");
        }
    }
}
