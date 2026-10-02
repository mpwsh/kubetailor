//! The two objects the flint controller reconciles (`flint.mpw.sh/v1`), as kubetailor uses them.
//! Only the fields kubetailor reads or writes are spelled out; the authoritative definitions and
//! their CRDs live in flint (`crates/core/src/crd.rs`, `flint controller --print-crds`).
//!
//! - A [`NodeClaim`] (cluster-scoped) asks for one node in a region. The operator creates them
//!   when an app is pinned to a region without a node, or when its pods cannot be scheduled for
//!   lack of room, and deletes the ones it created once the node has sat idle long enough.
//! - A [`FirewallRule`] (namespaced) opens a port on every node. The operator keeps one per port
//!   an app exposes at the node, owned by the app so it goes away with it.

use kube::{CustomResource, ResourceExt};
use schemars::JsonSchema;

use crate::prelude::*;

/// Label on the claims kubetailor created, so it never touches claims made by hand.
pub const MANAGED_LABEL: &str = "kubetailor.io/managed";
/// Label with the claim's region, for listing the claims of a region.
pub const CLAIM_REGION_LABEL: &str = "kubetailor.io/region";
/// Annotation set on a managed claim when its node was last seen without app pods (RFC 3339).
pub const IDLE_SINCE_ANNOTATION: &str = "kubetailor.io/idle-since";

/// A request for one node in a region.
#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
    group = "flint.mpw.sh",
    version = "v1",
    kind = "NodeClaim",
    plural = "nodeclaims",
    status = "NodeClaimStatus",
    derive = "PartialEq"
)]
#[serde(rename_all = "camelCase")]
pub struct NodeClaimSpec {
    pub region: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
}

/// Where a claim is in its life, as the flint controller reports it.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, JsonSchema, Default)]
pub enum NodeClaimPhase {
    #[default]
    Pending,
    Provisioning,
    Ready,
    Failed,
    Deleting,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct NodeClaimStatus {
    #[serde(default)]
    pub phase: NodeClaimPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// After a failure, when (RFC 3339) the controller tries again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<String>,
}

impl NodeClaim {
    /// The region the claim is for.
    pub fn region(&self) -> &str {
        &self.spec.region
    }

    /// Whether kubetailor made this claim (and may delete it).
    pub fn is_managed(&self) -> bool {
        self.labels().get(MANAGED_LABEL).map(String::as_str) == Some("true")
    }

    pub fn phase(&self) -> NodeClaimPhase {
        self.status.as_ref().map(|s| s.phase).unwrap_or_default()
    }

    /// The node behind the claim, once there is one.
    pub fn node(&self) -> Option<&str> {
        self.status.as_ref().and_then(|s| s.node.as_deref())
    }

    /// A one-line account of the claim for an app's status: the node when there is one, the
    /// controller's message otherwise.
    pub fn describe(&self) -> String {
        let name = self.name_any();
        match (
            self.phase(),
            self.node(),
            self.status.as_ref().and_then(|s| s.message.clone()),
        ) {
            (NodeClaimPhase::Ready, Some(node), _) => format!("node {node} ({name}) is ready"),
            (phase, Some(node), msg) => match msg {
                Some(m) => format!("node {node} is {phase:?}: {m}"),
                None => format!("node {node} is {phase:?}"),
            },
            (phase, None, Some(m)) => format!("claim {name} is {phase:?}: {m}"),
            (phase, None, None) => format!("claim {name} is {phase:?}"),
        }
    }
}

/// Transport protocol of a firewall rule, as flint spells it (lowercase).
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum RuleProtocol {
    #[default]
    Tcp,
    Udp,
}

impl From<Protocol> for RuleProtocol {
    fn from(p: Protocol) -> Self {
        match p {
            Protocol::Tcp => RuleProtocol::Tcp,
            Protocol::Udp => RuleProtocol::Udp,
        }
    }
}

/// A request to open a port on every node of the cluster.
#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
    group = "flint.mpw.sh",
    version = "v1",
    kind = "FirewallRule",
    plural = "firewallrules",
    status = "FirewallRuleStatus",
    derive = "PartialEq",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct FirewallRuleSpec {
    #[serde(default)]
    pub protocol: RuleProtocol,
    /// Port or inclusive range (`7777`, `27000-27100`).
    pub port: String,
    #[serde(default = "everyone")]
    pub cidr: String,
}

fn everyone() -> String {
    "0.0.0.0/0".to_owned()
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct FirewallRuleStatus {
    #[serde(default)]
    pub applied: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_status_as_flint_writes_it() {
        let claim: NodeClaim = serde_json::from_value(serde_json::json!({
            "apiVersion": "flint.mpw.sh/v1",
            "kind": "NodeClaim",
            "metadata": {"name": "region-sao", "labels": {MANAGED_LABEL: "true"}},
            "spec": {"region": "sao"},
            "status": {"phase": "Provisioning", "node": "kt-sao-4js7", "plan": "vc2-2c-4gb",
                       "message": "creating kt-sao-4js7, a vc2-2c-4gb node in sao",
                       "observedGeneration": 1}
        }))
        .unwrap();
        assert!(claim.is_managed());
        assert_eq!(claim.phase(), NodeClaimPhase::Provisioning);
        assert_eq!(claim.node(), Some("kt-sao-4js7"));
        assert!(claim
            .describe()
            .starts_with("node kt-sao-4js7 is Provisioning: creating"));

        let denied: NodeClaim = serde_json::from_value(serde_json::json!({
            "apiVersion": "flint.mpw.sh/v1", "kind": "NodeClaim",
            "metadata": {"name": "region-waw"},
            "spec": {"region": "waw"},
            "status": {"phase": "Failed", "message": "denied: region `waw` is not allowed"}
        }))
        .unwrap();
        assert!(!denied.is_managed());
        assert_eq!(
            denied.describe(),
            "claim region-waw is Failed: denied: region `waw` is not allowed"
        );
        let bare: NodeClaim = serde_json::from_value(serde_json::json!({
            "apiVersion": "flint.mpw.sh/v1", "kind": "NodeClaim",
            "metadata": {"name": "x"}, "spec": {"region": "sao"}
        }))
        .unwrap();
        assert_eq!(bare.phase(), NodeClaimPhase::Pending);
    }

    #[test]
    fn firewall_rule_serialises_for_flint() {
        let rule = FirewallRule::new(
            "game-udp-27015",
            FirewallRuleSpec {
                protocol: Protocol::Udp.into(),
                port: "27015".into(),
                cidr: everyone(),
            },
        );
        let json = serde_json::to_value(&rule).unwrap();
        assert_eq!(json["apiVersion"], "flint.mpw.sh/v1");
        assert_eq!(json["spec"]["protocol"], "udp");
        assert_eq!(json["spec"]["port"], "27015");
        assert_eq!(json["spec"]["cidr"], "0.0.0.0/0");
    }
}
