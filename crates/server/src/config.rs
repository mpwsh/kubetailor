use std::{env, fs, io::Read};

use serde::{Deserialize, Serialize};

use crate::{deployment::Deployment, ingress::Ingress};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Config {
    pub server: Server,
    pub kubetailor: Kubetailor,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Kubetailor {
    pub namespace: String,
    pub deployment: Deployment,
    pub ingress: Ingress,
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
