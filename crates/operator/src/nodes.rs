//! Nodes for the apps, through the flint controller's objects.
//!
//! kubetailor never talks to a cloud. When an app needs a node it does not have — it is pinned to
//! a region with no node, or the scheduler says its region is full — the operator creates a
//! `NodeClaim` and the flint controller does the rest; the app's status says what is going on
//! meanwhile. Ports an app exposes at the node become `FirewallRule`s owned by the app. And a
//! reaper gives back nodes kubetailor asked for once nothing has run on them for a while.
//!
//! Everything here is skipped, with one warning at start-up, on a cluster without the flint
//! CRDs: apps still deploy, they just cannot grow the cluster.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use kubetailor::{
    flint_crd::{
        FirewallRuleSpec, NodeClaimPhase, NodeClaimSpec, CLAIM_REGION_LABEL, IDLE_SINCE_ANNOTATION,
        MANAGED_LABEL,
    },
    k8s_openapi::api::core::v1::{Node, Pod},
};
use serde_json::json;

use crate::{
    apply::{apply, prune},
    deployment::REGION_LABEL,
    placement::allocated_node_ports,
    prelude::*,
};

/// How long the scheduler must have refused a pod before the region counts as full. Pods are
/// normally placed within seconds; this keeps a node that is just finishing a rollout from
/// looking like missing capacity.
const UNSCHEDULABLE_GRACE: Duration = Duration::from_secs(30);
/// Minutes a kubetailor-made node may sit without app pods before its claim is deleted.
pub const IDLE_MINUTES_ENV: &str = "KUBETAILOR_NODE_IDLE_MINUTES";
const DEFAULT_IDLE_MINUTES: u64 = 60;
/// Region for apps that name none when their pods need a node that does not exist yet. The
/// control plane's region when unset.
pub const DEFAULT_REGION_ENV: &str = "KUBETAILOR_DEFAULT_REGION";
/// Seconds between looks for idle nodes (default 300). Only worth lowering in a test.
pub const REAP_SECONDS_ENV: &str = "KUBETAILOR_NODE_REAP_SECONDS";
const DEFAULT_REAP_SECONDS: u64 = 5 * 60;
/// Most claims kubetailor makes for one region; the flint controller's policy is the real cap,
/// this only bounds the names.
const MAX_CLAIMS_PER_REGION: usize = 9;

/// The API group of the flint objects, as the discovery endpoint names it.
const FLINT_GROUP_VERSION: &str = "flint.mpw.sh/v1";

/// Whether the cluster has the flint CRDs, i.e. a flint controller to ask for nodes.
pub async fn available(client: &Client) -> bool {
    match client.list_api_group_resources(FLINT_GROUP_VERSION).await {
        Ok(list) => {
            let kinds: BTreeSet<&str> = list.resources.iter().map(|r| r.kind.as_str()).collect();
            kinds.contains("NodeClaim") && kinds.contains("FirewallRule")
        }
        Err(_) => false,
    }
}

/// How often idle nodes are looked for.
pub fn reap_interval() -> Duration {
    let secs = std::env::var(REAP_SECONDS_ENV)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(DEFAULT_REAP_SECONDS);
    Duration::from_secs(secs)
}

/// The idle window from the environment.
pub fn idle_window() -> Duration {
    let minutes = std::env::var(IDLE_MINUTES_ENV)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_IDLE_MINUTES);
    Duration::from_secs(minutes * 60)
}

/// Why an app cannot get a pod onto a node in its region.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Need {
    /// The region has no node at all.
    NoNode,
    /// Every node in the region is full, in the scheduler's words.
    NoRoom(String),
}

impl std::fmt::Display for Need {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Need::NoNode => write!(f, "no node in the region"),
            Need::NoRoom(why) => write!(f, "no room in the region ({why})"),
        }
    }
}

/// Whether the scheduler's reason for not placing a pod is one more node would fix. Resource
/// shortage, and host ports already taken (two copies of a game server on one node).
fn is_capacity_message(message: &str) -> bool {
    [
        "Insufficient cpu",
        "Insufficient memory",
        "Insufficient ephemeral-storage",
        "didn't have free ports",
    ]
    .iter()
    .any(|m| message.contains(m))
}

/// The scheduler's verdict on a pod that has been unschedulable for lack of room longer than
/// the grace period, if that is the pod's state.
fn capacity_shortage(pod: &Pod, now: DateTime<Utc>) -> Option<String> {
    if pod.metadata.deletion_timestamp.is_some() {
        return None;
    }
    let status = pod.status.as_ref()?;
    if status.phase.as_deref() != Some("Pending") {
        return None;
    }
    let cond = status
        .conditions
        .as_ref()?
        .iter()
        .find(|c| c.type_ == "PodScheduled" && c.status == "False")?;
    if cond.reason.as_deref() != Some("Unschedulable") {
        return None;
    }
    let message = cond.message.as_deref()?;
    if !is_capacity_message(message) {
        return None;
    }
    let since = cond.last_transition_time.as_ref()?.0;
    let waited = (now - since).to_std().unwrap_or_default();
    (waited >= UNSCHEDULABLE_GRACE).then(|| message.trim().to_owned())
}

/// The next free claim name for a region: `region-<r>`, then `region-<r>-2`, `-3`, …
fn next_claim_name(region: &str, taken: &BTreeSet<String>) -> Option<String> {
    let base = format!("region-{region}");
    if !taken.contains(&base) {
        return Some(base);
    }
    (2..=MAX_CLAIMS_PER_REGION)
        .map(|n| format!("{base}-{n}"))
        .find(|name| !taken.contains(name))
}

/// Whether a claim's node has been idle long enough to go, given when it was first seen idle.
fn idle_long_enough(idle_since: Option<&str>, now: DateTime<Utc>, window: Duration) -> bool {
    idle_since
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|since| {
            (now - since.with_timezone(&Utc))
                .to_std()
                .unwrap_or_default()
                >= window
        })
        .unwrap_or(false)
}

async fn nodes_in(client: &Client, region: &str) -> Result<Vec<Node>, Error> {
    Ok(Api::<Node>::all(client.clone())
        .list(&ListParams::default().labels(&format!("{REGION_LABEL}={region}")))
        .await?
        .items)
}

/// Region for an app that names none: the configured default, else the control plane's.
async fn default_region(client: &Client) -> Result<Option<String>, Error> {
    if let Ok(region) = std::env::var(DEFAULT_REGION_ENV) {
        if !region.trim().is_empty() {
            return Ok(Some(region.trim().to_owned()));
        }
    }
    let nodes = Api::<Node>::all(client.clone())
        .list(&ListParams::default().labels("node-role.kubernetes.io/control-plane"))
        .await?;
    Ok(nodes
        .items
        .iter()
        .find_map(|n| n.metadata.labels.as_ref()?.get(REGION_LABEL).cloned()))
}

/// A pod of this app the scheduler has been refusing for lack of room, if any.
async fn unschedulable_for_capacity(
    client: &Client,
    meta: &TappMeta,
) -> Result<Option<String>, Error> {
    let pods = Api::<Pod>::namespaced(client.clone(), &meta.namespace)
        .list(&ListParams::default().labels(&meta.label_selector()))
        .await?;
    let now = Utc::now();
    Ok(pods.items.iter().find_map(|p| capacity_shortage(p, now)))
}

/// Makes sure a node is on its way for the app when it needs one it does not have. Returns what
/// to tell the app's status: `None` when nothing is missing, otherwise the situation and what
/// was done about it.
pub async fn ensure_capacity(
    client: &Client,
    meta: &TappMeta,
    app: &TailoredApp,
) -> Result<Option<String>, Error> {
    let pinned = app.spec.deployment.region.as_deref().map(str::trim);
    if let Some(region) = pinned {
        if nodes_in(client, region).await?.is_empty() {
            return Ok(Some(
                request_node(client, meta, region, Need::NoNode).await?,
            ));
        }
    }
    let Some(why) = unschedulable_for_capacity(client, meta).await? else {
        return Ok(None);
    };
    let region = match pinned {
        Some(r) => r.to_owned(),
        None => match default_region(client).await? {
            Some(r) => r,
            None => {
                return Ok(Some(format!(
                    "{}; no default region to add a node in (set {DEFAULT_REGION_ENV})",
                    Need::NoRoom(why)
                )))
            }
        },
    };
    Ok(Some(
        request_node(client, meta, &region, Need::NoRoom(why)).await?,
    ))
}

/// One more node for `region`, unless one is already on its way (or refused): a claim that is
/// not Ready, made by kubetailor or by hand, is that node, and the app's status reports on it.
async fn request_node(
    client: &Client,
    meta: &TappMeta,
    region: &str,
    need: Need,
) -> Result<String, Error> {
    let api: Api<NodeClaim> = Api::all(client.clone());
    let claims = api.list(&ListParams::default()).await?.items;
    let taken: BTreeSet<String> = claims.iter().map(|c| c.name_any()).collect();
    let in_region: Vec<&NodeClaim> = claims.iter().filter(|c| c.region() == region).collect();
    if let Some(pending) = in_region
        .iter()
        .find(|c| c.phase() != NodeClaimPhase::Ready && c.metadata.deletion_timestamp.is_none())
    {
        return Ok(format!("{need}; {}", pending.describe()));
    }
    let Some(name) = next_claim_name(region, &taken) else {
        return Ok(format!(
            "{need}; already {MAX_CLAIMS_PER_REGION} claims for {region}, not asking for more"
        ));
    };
    let claim = NodeClaim {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            labels: Some(BTreeMap::from([
                (MANAGED_LABEL.to_owned(), "true".to_owned()),
                (CLAIM_REGION_LABEL.to_owned(), region.to_owned()),
            ])),
            ..ObjectMeta::default()
        },
        spec: NodeClaimSpec {
            region: region.to_owned(),
            plan: None,
            labels: BTreeMap::new(),
        },
        status: None,
    };
    info!(
        "{}: {need}; asking flint for a node in {region} ({name})",
        meta.name
    );
    match api.create(&PostParams::default(), &claim).await {
        Ok(_) => {}
        // Another app's reconcile got there first.
        Err(kubetailor::kube::Error::Api(e)) if e.code == 409 => {}
        Err(e) => return Err(Error::KubeError { source: e }),
    }
    Ok(format!(
        "{need}; asked flint for a node in {region} ({name})"
    ))
}

/// One `FirewallRule` per port the app exposes at a node, owned by the app. A `nodePort` port
/// gets its rule once Kubernetes has allocated the port; until then the next reconcile tries
/// again. Rules for ports the spec dropped are pruned.
pub async fn ensure_firewall_rules(
    client: &Client,
    meta: &TappMeta,
    app: &TailoredApp,
) -> Result<(), Error> {
    let mut keep = BTreeSet::new();
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
            Expose::Node => port.port,
            Expose::NodePort => {
                match node_ports.get(&(port.port, port.protocol.as_str().to_owned())) {
                    Some(p) => *p,
                    None => continue,
                }
            }
            Expose::Cluster => continue,
        };
        let name = format!(
            "{}-{}-{client_port}",
            meta.name,
            port.protocol.as_str().to_lowercase()
        );
        let rule = FirewallRule {
            metadata: ObjectMeta {
                name: Some(name.clone()),
                namespace: Some(meta.namespace.clone()),
                labels: Some(meta.labels.clone()),
                owner_references: Some(vec![meta.oref.clone()]),
                ..ObjectMeta::default()
            },
            spec: FirewallRuleSpec {
                protocol: port.protocol.into(),
                port: client_port.to_string(),
                cidr: "0.0.0.0/0".to_owned(),
            },
            status: None,
        };
        apply(client, &meta.namespace, &rule).await?;
        keep.insert(name);
    }
    prune::<FirewallRule>(client, meta, &keep).await
}

/// App pods (anything with a `tapp` label) currently on `node`, terminating ones excluded.
async fn app_pods_on(client: &Client, node: &str) -> Result<usize, Error> {
    let pods = Api::<Pod>::all(client.clone())
        .list(
            &ListParams::default()
                .labels("tapp")
                .fields(&format!("spec.nodeName={node}")),
        )
        .await?;
    Ok(pods
        .items
        .iter()
        .filter(|p| p.metadata.deletion_timestamp.is_none())
        .count())
}

/// Deletes the claims kubetailor made whose node has had no app pods for the idle window,
/// which has flint drain and destroy the node. A claim without a node (refused, or still
/// pending) goes the same way once no app asks for its region any more. Claims made by hand are
/// never touched. Meant to run every [`REAP_INTERVAL`].
pub async fn reap_idle(client: &Client, window: Duration) -> Result<(), Error> {
    let api: Api<NodeClaim> = Api::all(client.clone());
    let claims = api
        .list(&ListParams::default().labels(&format!("{MANAGED_LABEL}=true")))
        .await?
        .items;
    if claims.is_empty() {
        return Ok(());
    }
    let wanted: BTreeSet<String> = Api::<TailoredApp>::all(client.clone())
        .list(&ListParams::default())
        .await?
        .items
        .iter()
        .filter_map(|a| a.spec.deployment.region.clone())
        .collect();
    let now = Utc::now();
    for claim in claims {
        if claim.metadata.deletion_timestamp.is_some() {
            continue;
        }
        let name = claim.name_any();
        let idle = match claim.node() {
            Some(node) => app_pods_on(client, node).await? == 0,
            None => {
                claim.phase() != NodeClaimPhase::Provisioning && !wanted.contains(claim.region())
            }
        };
        let since = claim
            .annotations()
            .get(IDLE_SINCE_ANNOTATION)
            .map(String::as_str);
        match (idle, since) {
            (false, None) => {}
            (false, Some(_)) => {
                api.patch(
                    &name,
                    &PatchParams::default(),
                    &Patch::Merge(
                        json!({"metadata": {"annotations": {IDLE_SINCE_ANNOTATION: null}}}),
                    ),
                )
                .await?;
            }
            (true, None) => {
                info!(
                    "node claim {name} ({}) has no app pods; idle from now",
                    claim.region()
                );
                api.patch(
                    &name,
                    &PatchParams::default(),
                    &Patch::Merge(json!({"metadata": {"annotations": {
                        IDLE_SINCE_ANNOTATION: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
                    }}})),
                )
                .await?;
            }
            (true, Some(since)) if idle_long_enough(Some(since), now, window) => {
                info!(
                    "node claim {name} ({}) idle since {since}; deleting it",
                    claim.region()
                );
                match api.delete(&name, &DeleteParams::default()).await {
                    Ok(_) => {}
                    Err(kubetailor::kube::Error::Api(e)) if e.code == 404 => {}
                    Err(e) => return Err(Error::KubeError { source: e }),
                }
            }
            (true, Some(_)) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use kubetailor::k8s_openapi::{
        api::core::v1::{PodCondition, PodStatus},
        apimachinery::pkg::apis::meta::v1::Time,
    };

    use super::*;

    fn pending_pod(reason: &str, message: &str, age_secs: i64) -> Pod {
        Pod {
            status: Some(PodStatus {
                phase: Some("Pending".into()),
                conditions: Some(vec![PodCondition {
                    type_: "PodScheduled".into(),
                    status: "False".into(),
                    reason: Some(reason.into()),
                    message: Some(message.into()),
                    last_transition_time: Some(Time(
                        Utc::now() - chrono::Duration::seconds(age_secs),
                    )),
                    ..PodCondition::default()
                }]),
                ..PodStatus::default()
            }),
            ..Pod::default()
        }
    }

    #[test]
    fn only_room_shortages_older_than_the_grace_count() {
        let now = Utc::now();
        let full = pending_pod(
            "Unschedulable",
            "0/2 nodes are available: 1 Insufficient cpu, 1 node(s) didn't match Pod's node affinity/selector.",
            60,
        );
        assert!(capacity_shortage(&full, now)
            .unwrap()
            .contains("Insufficient cpu"));
        let ports = pending_pod(
            "Unschedulable",
            "0/1 nodes are available: 1 node(s) didn't have free ports for the requested pod ports.",
            60,
        );
        assert!(capacity_shortage(&ports, now).is_some());
        // Fresh: the scheduler may still be catching up with a node that just joined.
        let fresh = pending_pod(
            "Unschedulable",
            "0/2 nodes are available: 2 Insufficient memory.",
            5,
        );
        assert_eq!(capacity_shortage(&fresh, now), None);
        // No node in the region: not a room problem, handled by the node listing instead.
        let no_node = pending_pod(
            "Unschedulable",
            "0/1 nodes are available: 1 node(s) didn't match Pod's node affinity/selector.",
            60,
        );
        assert_eq!(capacity_shortage(&no_node, now), None);
        // A node that is NotReady is not missing capacity either.
        let not_ready = pending_pod(
            "Unschedulable",
            "0/2 nodes are available: 1 node(s) had untolerated taint {node.kubernetes.io/not-ready: }, 1 Insufficient cpu.",
            60,
        );
        assert!(
            capacity_shortage(&not_ready, now).is_some(),
            "mixed: the full node still counts"
        );
        let mut running = pending_pod("Unschedulable", "1 Insufficient cpu", 60);
        running.status.as_mut().unwrap().phase = Some("Running".into());
        assert_eq!(capacity_shortage(&running, now), None);
    }

    #[test]
    fn claim_names_fill_in_order() {
        let mut taken = BTreeSet::new();
        assert_eq!(
            next_claim_name("sao", &taken).as_deref(),
            Some("region-sao")
        );
        taken.insert("region-sao".into());
        assert_eq!(
            next_claim_name("sao", &taken).as_deref(),
            Some("region-sao-2")
        );
        taken.insert("region-sao-2".into());
        taken.insert("region-sao-3".into());
        assert_eq!(
            next_claim_name("sao", &taken).as_deref(),
            Some("region-sao-4")
        );
        for n in 4..=MAX_CLAIMS_PER_REGION {
            taken.insert(format!("region-sao-{n}"));
        }
        assert_eq!(next_claim_name("sao", &taken), None);
        assert_eq!(
            next_claim_name("waw", &taken).as_deref(),
            Some("region-waw")
        );
    }

    #[test]
    fn idle_window_is_measured_from_the_annotation() {
        let now = Utc::now();
        let window = Duration::from_secs(3600);
        let old = (now - chrono::Duration::minutes(61)).to_rfc3339();
        let recent = (now - chrono::Duration::minutes(10)).to_rfc3339();
        assert!(idle_long_enough(Some(&old), now, window));
        assert!(!idle_long_enough(Some(&recent), now, window));
        assert!(!idle_long_enough(None, now, window));
        assert!(!idle_long_enough(Some("not a time"), now, window));
    }

    #[test]
    fn need_reads_well_in_a_status() {
        assert_eq!(Need::NoNode.to_string(), "no node in the region");
        assert_eq!(
            Need::NoRoom("0/2 nodes are available: 2 Insufficient cpu.".into()).to_string(),
            "no room in the region (0/2 nodes are available: 2 Insufficient cpu.)"
        );
    }
}
