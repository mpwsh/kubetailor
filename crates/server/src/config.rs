use std::{env, fmt, fs, io::Read};

use kubetailor::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{Container, Deployment, Domains, Ingress};
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Config {
    pub server: ServerConfig,
    pub kubetailor: KubetailorConfig,
}
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct KubetailorConfig {
    #[serde(rename = "baseDomain")]
    pub base_domain: String,
    pub namespace: String,
    deployment: DeploymentConfig,
    ingress: IngressConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ServerConfig {
    #[serde(rename = "logLevel")]
    pub log_level: String,
    pub addr: String,
    pub port: i32,
}
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct DeploymentConfig {
    annotations: BTreeMap<String, String>,
    container: Option<Container>,
    replicas: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct IngressConfig {
    annotations: BTreeMap<String, String>,
    load_balancer_endpoint: String,
    class_name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TailoredAppConfig {
    pub name: String,
    pub owner: String,
    pub domains: Domains,
    pub container: Container,
    #[serde(skip_deserializing, skip_serializing)]
    pub labels: BTreeMap<String, String>,
    pub config: BTreeMap<String, String>,
    #[serde(skip_deserializing, skip_serializing)]
    pub secrets: BTreeMap<String, String>,
    #[serde(skip_deserializing, skip_serializing)]
    pub kubetailor: KubetailorConfig,
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
impl KubetailorConfig {
    pub fn build_deployment(&self, container: &Container) -> Deployment {
        let mut annotations = BTreeMap::new();
        let container = if let Some(container) = self.deployment.container.clone() {
            Container {
                image: container.image,
                port: container.port,
                replicas: 1,
                build_command: container.build_command,
                run_command: container.run_command,
                git_repository: container.git_repository,
                volumes: BTreeMap::new(),
                file_mounts: BTreeMap::new(),
            }
        } else {
            container.clone()
        };

        for a in self.deployment.annotations.iter() {
            annotations.insert(a.0.to_owned(), a.1.to_owned());
        }
        Deployment {
            annotations,
            container,
        }
    }
    pub fn build_ingress(&self, subdomain: String, custom: Option<String>) -> Ingress {
        Ingress {
            annotations: self
                .ingress
                .build_annotations(&subdomain, &self.base_domain),
            class_name: self.ingress.class_name.clone(),
            domains: Domains {
                shared: format!("{}.{}", subdomain, self.base_domain),
                custom,
            },
        }
    }
}

impl IngressConfig {
    pub fn build_annotations(
        &self,
        subdomain: &str,
        base_domain: &str,
    ) -> BTreeMap<String, String> {
        let mut annotations = BTreeMap::new();
        for a in self.annotations.iter() {
            annotations.insert(a.0.to_owned(), a.1.to_owned());
        }
        annotations.insert(
            "external-dns.alpha.kubernetes.io/target".to_owned(),
            self.load_balancer_endpoint.clone(),
        );
        annotations.insert(
            "external-dns.alpha.kubernetes.io/hostname".to_owned(),
            format!("{}.{}", subdomain, base_domain),
        );
        annotations
    }
}

impl TailoredAppConfig {
    pub fn sanitize_input(&mut self) {
        let sanitized_config: BTreeMap<String, String> = self
            .config
            .clone()
            .into_iter()
            .map(|(key, value)| {
                let sanitized_key = key
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect::<String>();

                (sanitized_key, value)
            })
            .collect();
        self.config = sanitized_config;
    }
}

#[derive(Debug)]
pub enum TailoredAppError {
    InvalidDomain(String),
    InvalidName(String),
}

impl fmt::Display for TailoredAppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TailoredAppError::InvalidDomain(msg) => write!(f, "Invalid domain: {}", msg),
            TailoredAppError::InvalidName(msg) => write!(f, "Invalid tapp name: {}", msg),
        }
    }
}

impl TryFrom<TailoredAppConfig> for TailoredApp {
    type Error = TailoredAppError;

    fn try_from(mut app: TailoredAppConfig) -> Result<Self, Self::Error> {
        app.sanitize_input();
        //parse shared subdomain
        let re = Regex::new(r"^[a-z0-9]([a-z0-9-]{1,61}[a-z0-9])$").unwrap();
        if !re.is_match(&app.domains.shared) {
            return Err(TailoredAppError::InvalidDomain(app.domains.shared));
        };
        if !re.is_match(&app.name) {
            return Err(TailoredAppError::InvalidName(app.name));
        };
        if let Some(custom) = app.domains.custom.clone() {
            //parse custom domain
            let re = Regex::new(r"^(?:[a-z0-9]([a-z0-9-]{1,61}[a-z0-9])?\.)?[a-z0-9]([a-z0-9-]{1,61}[a-z0-9])?\.(?:[a-z]{2,}\.)?[a-z]{2,}$").unwrap();
            if !re.is_match(&custom) {
                return Err(TailoredAppError::InvalidDomain(custom));
            };
        };
        let mut secrets = BTreeMap::new();
        secrets.insert("test-secret".to_owned(), "1234".to_owned());

        //Insert Owner to tapp label
        let mut labels = BTreeMap::new();
        labels.insert("owner".to_owned(), app.owner.replace('@', "-"));
        labels.insert("tapp".to_owned(), app.name.to_owned());

        let mut tapp = TailoredApp::new(&app.name.to_lowercase(), TailoredAppSpec {
            labels: labels.clone(),
            deployment: app.kubetailor.build_deployment(&app.container),
            ingress: app
                .kubetailor
                .build_ingress(app.domains.shared, app.domains.custom),
            config_map: app.config,
            secrets,
        });

        tapp.metadata.labels = Some(labels);
        Ok(tapp)
    }
}

impl TryFrom<TailoredApp> for TailoredAppConfig {
    type Error = TailoredAppError;

    fn try_from(tapp: TailoredApp) -> Result<Self, Self::Error> {
        let container = tapp.spec.deployment.container;
        let domains = tapp.spec.ingress.domains;
        let app_config = TailoredAppConfig {
            name: tapp.metadata.name.unwrap(),
            owner: String::new(),
            domains: Domains {
                shared: domains.shared,
                custom: domains.custom,
            },
            container: Container {
                image: container.image,
                port: container.port,
                replicas: 1,
                build_command: container.build_command,
                run_command: container.run_command,
                git_repository: container.git_repository,
                volumes: BTreeMap::new(),
                file_mounts: BTreeMap::new(),
            },
            config: tapp.spec.config_map,
            secrets: BTreeMap::new(),
            labels: BTreeMap::new(),
            kubetailor: KubetailorConfig::default(),
        };

        Ok(app_config)
    }
}
