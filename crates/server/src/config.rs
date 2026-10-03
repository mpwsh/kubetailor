use std::{env, fs, io::Read};

use serde::{Deserialize, Serialize};

use crate::{deployment::Deployment, git::Git, ingress::Ingress};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Config {
    pub server: Server,
    pub quickwit: Quickwit,
    pub kubetailor: Kubetailor,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Quickwit {
    pub index: String,
    pub url: String,
    pub api_version: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Kubetailor {
    pub namespace: String,
    pub deployment: Deployment,
    pub ingress: Ingress,
    pub git_sync: Git,
    /// Regions apps may be pinned to, with the name people see. Empty: whatever regions the
    /// cluster's nodes carry, named by their id.
    #[serde(default)]
    pub regions: Vec<Region>,
}

/// A region as the console offers it: the node label value and a display name.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct Region {
    /// The `topology.kubernetes.io/region` label value (`scl`, `waw`).
    pub id: String,
    /// What people see (`Santiago`, `Warsaw`).
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub log_level: String,
    pub addr: String,
    pub port: i32,
}

impl Config {
    pub fn load() -> Result<Config, Box<dyn std::error::Error>> {
        let config_path = env::var("CONFIG_PATH").unwrap_or("config.yaml".to_string());

        let mut file = fs::File::open(config_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let config: Config = serde_yaml::from_str(&contents)?;

        Ok(config)
    }
}
