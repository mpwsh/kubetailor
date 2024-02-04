pub use std::collections::HashMap;

pub use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Default)]
pub struct TappConfig {
    pub name: String,
    pub group: Option<String>,
    #[serde(skip_deserializing)]
    pub owner: String,
    pub domains: Domains,
    pub container: Container,
    pub git: Option<Git>,
    pub env: Option<HashMap<String, String>>,
    pub secrets: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct Domains {
    pub custom: Option<String>,
    pub shared: String,
}

#[derive(Deserialize, Serialize, Default)]
pub struct Git {
    #[serde(skip_serializing_if = "is_empty_string")]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "is_empty_string")]
    pub branch: Option<String>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct Container {
    pub image: String,
    pub replicas: u32,
    pub port: u32,
    pub volumes: Option<HashMap<String, String>>,
    pub files: Option<HashMap<String, String>>,
    #[serde(rename = "buildCommand", skip_serializing_if = "is_empty_string")]
    pub build_command: Option<String>,
    #[serde(rename = "runCommand", skip_serializing_if = "is_empty_string")]
    pub run_command: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
struct Metadata {
    name: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct Tapp {
    metadata: Metadata,
}
#[derive(Deserialize, Serialize, Debug)]
struct TappListResponse {
    metadata: Metadata,
}

fn is_empty_string(opt: &Option<String>) -> bool {
    matches!(opt, Some(s) if s.trim().is_empty())
}
