pub use std::collections::HashMap;

pub use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct TappConfig {
    pub name: String,
    pub group: Option<String>,
    #[serde(skip_deserializing)]
    pub owner: String,
    /// `None` for an app reached by IP only (no HTTP port, nothing for an ingress to route).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domains: Option<Domains>,
    pub container: Container,
    /// Region the app must run in (a node label); unset lets the scheduler choose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    pub git: Option<Git>,
    pub env: Option<HashMap<String, String>>,
    pub secrets: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Domains {
    pub custom: Option<String>,
    pub shared: String,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Git {
    #[serde(skip_serializing_if = "is_empty_string")]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "is_empty_string")]
    pub branch: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Container {
    pub image: String,
    pub replicas: u32,
    /// The HTTP port the ingress routes the app's domains to; `None` for apps without HTTP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u32>,
    /// Other ports, each with its protocol and how it is exposed — the only way to publish UDP.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<Port>,
    pub volumes: Option<HashMap<String, String>>,
    pub files: Option<HashMap<String, String>>,
    #[serde(rename = "buildCommand", skip_serializing_if = "is_empty_string")]
    pub build_command: Option<String>,
    #[serde(rename = "runCommand", skip_serializing_if = "is_empty_string")]
    pub run_command: Option<String>,
    /// What one instance needs (`250m`, `256Mi`). Unset lets the API apply its minimums.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<Resources>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Default)]
pub struct Resources {
    pub cpu: String,
    pub memory: String,
}

/// One extra container port, spelled as the API spells it: `protocol` is `TCP` or `UDP`,
/// `expose` is `cluster` (Service only), `node` (hostPort on the node's public IP, same port
/// number) or `nodePort` (NodePort Service, number assigned by Kubernetes).
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct Port {
    pub port: u32,
    #[serde(default = "Port::default_protocol")]
    pub protocol: String,
    #[serde(default = "Port::default_expose")]
    pub expose: String,
}

impl Port {
    pub const PROTOCOLS: [&'static str; 2] = ["TCP", "UDP"];
    pub const EXPOSURES: [&'static str; 3] = ["cluster", "node", "nodePort"];

    fn default_protocol() -> String {
        "TCP".to_owned()
    }

    fn default_expose() -> String {
        "cluster".to_owned()
    }
}

/// What the API answers on `GET /config`: the facts the wizard needs before building a request.
/// Every field has a default so an older server (no such route) degrades to "unknown" rather
/// than a 500.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiConfig {
    /// Shared subdomains land under this (`<name>.<base_domain>`); empty when unknown.
    #[serde(default)]
    pub base_domain: String,
    /// Inclusive range for ports exposed at the node.
    #[serde(default = "ApiConfig::default_node_port_range")]
    pub node_port_range: (u32, u32),
    #[serde(default)]
    pub allowed_images: Option<Vec<String>>,
    /// Regions on offer, with how many nodes each has right now. Empty when the API does not
    /// say: the wizard then takes a region as free text.
    #[serde(default)]
    pub regions: Vec<RegionInfo>,
    /// Bounds on what one app may ask for; unset bounds are not enforced here (the API still
    /// has the last word).
    #[serde(default)]
    pub limits: Limits,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct RegionInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub nodes: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Default)]
pub struct Limits {
    #[serde(default)]
    pub cpu: Range,
    #[serde(default)]
    pub memory: Range,
    #[serde(default)]
    pub volume: VolumeLimits,
}

/// A bound as the API spells it: a Kubernetes quantity, either end optional.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Default)]
pub struct Range {
    #[serde(default)]
    pub min: Option<String>,
    #[serde(default)]
    pub max: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Default)]
pub struct VolumeLimits {
    #[serde(default)]
    pub count: CountRange,
    #[serde(default)]
    pub size: Range,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Default)]
pub struct CountRange {
    #[serde(default)]
    pub min: Option<usize>,
    #[serde(default)]
    pub max: Option<usize>,
}

impl ApiConfig {
    fn default_node_port_range() -> (u32, u32) {
        (1024, 29999)
    }

    /// The region as people see it, for an id the API offers; the id itself otherwise.
    pub fn region_name(&self, id: &str) -> String {
        self.regions
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.name.clone())
            .unwrap_or_else(|| id.to_owned())
    }

    /// Whether a region has no node yet, as far as the API knows. Unknown regions are not
    /// claimed to be empty.
    pub fn region_is_empty(&self, id: &str) -> bool {
        self.regions.iter().any(|r| r.id == id && r.nodes == 0)
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        ApiConfig {
            base_domain: String::new(),
            node_port_range: Self::default_node_port_range(),
            allowed_images: None,
            regions: Vec::new(),
            limits: Limits::default(),
        }
    }
}

fn is_empty_string(opt: &Option<String>) -> bool {
    matches!(opt, Some(s) if s.trim().is_empty())
}

#[derive(Serialize, Deserialize)]
pub struct Action {
    pub name: String,
    pub url: String,
    pub is_form: bool,
}

impl Action {
    pub fn new(name: &str) -> Self {
        Action {
            name: name.to_string(),
            url: String::new(), // Initialize with an empty URL
            is_form: false,
        }
    }

    pub fn url(mut self, url: &str) -> Self {
        self.url = url.to_string();
        self
    }
    pub fn form(mut self) -> Self {
        self.is_form = true;
        self
    }
}
