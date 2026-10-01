//! Where the app actually runs.
//!
//! For apps that are bound to a node — a `region` was requested, or a port is exposed straight at
//! the node — the operator resolves the node(s) hosting the pods to their public IPs, writes them
//! to the TailoredApp status (so "where is my server?" is a `kubectl get tapp` away) and points
//! the app's DNS record at them through the external-dns `target` annotation. This runs on every
//! reconcile, because pods get scheduled some time after the Deployment is created.

use std::collections::BTreeSet;

use kubetailor::{
    crd::{Endpoint, Expose, PlacementNode, TailoredAppStatus},
    k8s_openapi::api::core::v1::{Node, Pod},
};
use serde_json::json;

use crate::{deployment::REGION_LABEL, prelude::*, service::node_service_name};

pub const DNS_TARGET_ANNOTATION: &str = "external-dns.alpha.kubernetes.io/target";
pub const DNS_HOSTNAME_ANNOTATION: &str = "external-dns.alpha.kubernetes.io/hostname";

/// Node label holding the node's public IPv4. flint sets `flint.mpw.sh/public-ip`; override with
/// this environment variable for nodes provisioned some other way.
pub const PUBLIC_IP_LABEL_ENV: &str = "KUBETAILOR_PUBLIC_IP_LABEL";
pub const DEFAULT_PUBLIC_IP_LABEL: &str = "flint.mpw.sh/public-ip";

fn public_ip_label() -> String {
    std::env::var(PUBLIC_IP_LABEL_ENV).unwrap_or_else(|_| DEFAULT_PUBLIC_IP_LABEL.to_owned())
}

/// The address clients on the internet reach this node at: the public-ip label, else the node's
/// ExternalIP, else its InternalIP (correct on clouds that put the public address on the NIC).
fn public_ip(node: &Node, label: &str) -> Option<String> {
    if let Some(ip) = node.metadata.labels.as_ref().and_then(|l| l.get(label)) {
        return Some(ip.clone());
    }
    let addresses = node.status.as_ref()?.addresses.as_ref()?;
    ["ExternalIP", "InternalIP"].iter().find_map(|kind| {
        addresses
            .iter()
            .find(|a| a.type_ == *kind)
            .map(|a| a.address.clone())
    })
}

/// Nodes currently running a scheduled, non-terminating pod of the app.
async fn placed_nodes(client: &Client, meta: &TappMeta) -> Result<Vec<PlacementNode>, Error> {
    let pods: Api<Pod> = Api::namespaced(client.clone(), &meta.namespace);
    let pods = pods
        .list(&ListParams::default().labels(&meta.label_selector()))
        .await?;
    let node_names: BTreeSet<String> = pods
        .items
        .into_iter()
        .filter(|p| p.metadata.deletion_timestamp.is_none())
        .filter_map(|p| p.spec.and_then(|s| s.node_name))
        .collect();

    let nodes: Api<Node> = Api::all(client.clone());
    let label = public_ip_label();
    let mut placed = Vec::new();
    for name in node_names {
        let node = nodes.get(&name).await?;
        let Some(ip) = public_ip(&node, &label) else {
            warn!("node {name} has neither a {label} label nor an address; skipping");
            continue;
        };
        let region = node
            .metadata
            .labels
            .as_ref()
            .and_then(|l| l.get(REGION_LABEL).cloned());
        placed.push(PlacementNode { name, ip, region });
    }
    Ok(placed)
}

/// Node ports Kubernetes allocated for the `expose: nodePort` ports, keyed by (port, protocol).
async fn allocated_node_ports(
    client: &Client,
    meta: &TappMeta,
) -> Result<BTreeMap<(i32, String), i32>, Error> {
    let api: Api<Service> = Api::namespaced(client.clone(), &meta.namespace);
    let svc = match api.get(&node_service_name(meta)).await {
        Ok(svc) => svc,
        Err(kubetailor::kube::Error::Api(e)) if e.code == 404 => return Ok(BTreeMap::new()),
        Err(e) => return Err(Error::KubeError { source: e }),
    };
    Ok(svc
        .spec
        .and_then(|s| s.ports)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| {
            let node_port = p.node_port?;
            Some((
                (p.port, p.protocol.unwrap_or_else(|| "TCP".to_owned())),
                node_port,
            ))
        })
        .collect())
}

async fn desired_status(
    client: &Client,
    meta: &TappMeta,
    app: &TailoredApp,
) -> Result<TailoredAppStatus, Error> {
    let nodes = placed_nodes(client, meta).await?;
    let mut status = TailoredAppStatus::default();

    if nodes.is_empty() {
        status.message = Some(match &app.spec.deployment.region {
            Some(region) => {
                let in_region = Api::<Node>::all(client.clone())
                    .list(&ListParams::default().labels(&format!("{REGION_LABEL}={region}")))
                    .await?;
                if in_region.items.is_empty() {
                    format!("no node in region {region}")
                } else {
                    format!("waiting for a pod to be scheduled in region {region}")
                }
            }
            None => "waiting for a pod to be scheduled".to_owned(),
        });
        return Ok(status);
    }

    let node_ports = if app
        .spec
        .external_ports()
        .any(|p| p.expose == Expose::NodePort)
    {
        allocated_node_ports(client, meta).await?
    } else {
        BTreeMap::new()
    };

    for port in app.spec.external_ports() {
        let client_port = match port.expose {
            Expose::Node => Some(port.port),
            Expose::NodePort => node_ports
                .get(&(port.port, port.protocol.as_str().to_owned()))
                .copied(),
            Expose::Cluster => None,
        };
        let Some(client_port) = client_port else {
            continue;
        };
        for node in &nodes {
            status.endpoints.push(Endpoint {
                ip: node.ip.clone(),
                port: client_port,
                protocol: port.protocol,
            });
        }
    }
    status.nodes = nodes;
    Ok(status)
}

/// Points the app's DNS name at the nodes. The Ingress carries the annotation when there is one;
/// otherwise the in-cluster Service does, with the hostname too, for external-dns's service
/// source.
async fn point_dns(
    client: &Client,
    meta: &TappMeta,
    app: &TailoredApp,
    nodes: &[PlacementNode],
) -> Result<(), Error> {
    let hostnames = app.spec.hostnames();
    if hostnames.is_empty() || nodes.is_empty() {
        return Ok(());
    }
    let target = nodes
        .iter()
        .map(|n| n.ip.as_str())
        .collect::<Vec<_>>()
        .join(",");

    if app.spec.wants_ingress() {
        let api: Api<Ingress> = Api::namespaced(client.clone(), &meta.namespace);
        let current = api.get_opt(&meta.name).await?;
        let Some(current) = current else {
            return Ok(());
        };
        if annotation(&current.metadata, DNS_TARGET_ANNOTATION) == Some(target.as_str()) {
            return Ok(());
        }
        info!("{}: DNS target -> {target}", meta.name);
        api.patch(
            &meta.name,
            &PatchParams::default(),
            &Patch::Merge(json!({"metadata": {"annotations": {DNS_TARGET_ANNOTATION: target}}})),
        )
        .await?;
    } else {
        let api: Api<Service> = Api::namespaced(client.clone(), &meta.namespace);
        let Some(current) = api.get_opt(&meta.name).await? else {
            return Ok(());
        };
        let hostname = hostnames.join(",");
        if annotation(&current.metadata, DNS_TARGET_ANNOTATION) == Some(target.as_str())
            && annotation(&current.metadata, DNS_HOSTNAME_ANNOTATION) == Some(hostname.as_str())
        {
            return Ok(());
        }
        info!("{}: DNS {hostname} -> {target} (via service)", meta.name);
        api.patch(
            &meta.name,
            &PatchParams::default(),
            &Patch::Merge(json!({"metadata": {"annotations": {
                DNS_TARGET_ANNOTATION: target,
                DNS_HOSTNAME_ANNOTATION: hostname,
            }}})),
        )
        .await?;
    }
    Ok(())
}

fn annotation<'a>(meta: &'a ObjectMeta, key: &str) -> Option<&'a str> {
    meta.annotations
        .as_ref()
        .and_then(|a| a.get(key))
        .map(String::as_str)
}

/// Publishes the app's status: the generation just applied and, for node-bound apps, where the
/// pods run plus the DNS target. One status patch per change, none when nothing moved.
pub async fn publish(client: &Client, meta: &TappMeta, app: &TailoredApp) -> Result<(), Error> {
    let mut status = if app.spec.is_node_bound() {
        desired_status(client, meta, app).await?
    } else {
        TailoredAppStatus::default()
    };
    status.observed_generation = app.metadata.generation;

    if app.status.as_ref() != Some(&status) {
        patch_status(client, meta, &status).await?;
    }
    if app.spec.is_node_bound() {
        point_dns(client, meta, app, &status.nodes).await?;
    }
    Ok(())
}

/// Puts the reason an apply failed on the object (`status.message`), so `kubectl get tapp -o yaml`
/// explains itself. Best effort: a failure here is logged, the original error is what matters.
pub async fn report_error(client: &Client, meta: &TappMeta, error: &Error) {
    let api: Api<TailoredApp> = Api::namespaced(client.clone(), &meta.namespace);
    let patch = json!({"status": {"message": error.to_string()}});
    if let Err(e) = api
        .patch_status(&meta.name, &PatchParams::default(), &Patch::Merge(patch))
        .await
    {
        warn!("{}: could not record error in status: {e}", meta.name);
    }
}

async fn patch_status(
    client: &Client,
    meta: &TappMeta,
    status: &TailoredAppStatus,
) -> Result<(), Error> {
    let api: Api<TailoredApp> = Api::namespaced(client.clone(), &meta.namespace);
    // Every field spelled out (empty lists, explicit null): a merge patch only removes what it
    // names, and the serde representation skips empty values.
    let patch = json!({"status": {
        "observedGeneration": status.observed_generation,
        "nodes": status.nodes,
        "endpoints": status.endpoints,
        "message": status.message,
    }});
    api.patch_status(&meta.name, &PatchParams::default(), &Patch::Merge(patch))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use kubetailor::k8s_openapi::api::core::v1::{NodeAddress, NodeStatus};

    use super::*;

    fn node(labels: &[(&str, &str)], addresses: &[(&str, &str)]) -> Node {
        Node {
            metadata: ObjectMeta {
                labels: Some(
                    labels
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                ),
                ..ObjectMeta::default()
            },
            status: Some(NodeStatus {
                addresses: Some(
                    addresses
                        .iter()
                        .map(|(t, a)| NodeAddress {
                            type_: t.to_string(),
                            address: a.to_string(),
                        })
                        .collect(),
                ),
                ..NodeStatus::default()
            }),
            ..Node::default()
        }
    }

    #[test]
    fn label_wins_over_addresses() {
        let n = node(
            &[(DEFAULT_PUBLIC_IP_LABEL, "203.0.113.9")],
            &[("InternalIP", "10.0.0.5"), ("ExternalIP", "198.51.100.1")],
        );
        assert_eq!(
            public_ip(&n, DEFAULT_PUBLIC_IP_LABEL).as_deref(),
            Some("203.0.113.9")
        );
    }

    #[test]
    fn external_ip_before_internal_ip() {
        let n = node(
            &[],
            &[("InternalIP", "10.0.0.5"), ("ExternalIP", "198.51.100.1")],
        );
        assert_eq!(
            public_ip(&n, DEFAULT_PUBLIC_IP_LABEL).as_deref(),
            Some("198.51.100.1")
        );
        let n = node(&[], &[("InternalIP", "10.0.0.5")]);
        assert_eq!(
            public_ip(&n, DEFAULT_PUBLIC_IP_LABEL).as_deref(),
            Some("10.0.0.5")
        );
        assert_eq!(public_ip(&node(&[], &[]), DEFAULT_PUBLIC_IP_LABEL), None);
    }
}
