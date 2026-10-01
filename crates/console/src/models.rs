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
}

impl ApiConfig {
    fn default_node_port_range() -> (u32, u32) {
        (1024, 29999)
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        ApiConfig {
            base_domain: String::new(),
            node_port_range: Self::default_node_port_range(),
            allowed_images: None,
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
