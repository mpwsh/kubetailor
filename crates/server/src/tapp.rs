use std::collections::BTreeMap;
use kubetailor::crd::{Container, Domains, TailoredApp, TailoredAppSpec};
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::{deployment::Deployment, config::Kubetailor, error::TappRequestError};

#[derive(Serialize, Deserialize, Debug)]
pub struct TappRequest {
    pub name: String,
    pub owner: String,
    pub domains: Domains,
    pub container: Container,
    //#[serde(skip_deserializing, skip_serializing)]
    //pub labels: BTreeMap<String, String>,
    pub env_vars: BTreeMap<String, String>,
    #[serde(skip_deserializing, skip_serializing)]
    pub secrets: BTreeMap<String, String>,
    #[serde(skip_deserializing, skip_serializing)]
    pub kubetailor: Kubetailor,
}

impl TappRequest {
    pub fn sanitize_input(&mut self) {
        let sanitized_config: BTreeMap<String, String> = self
            .env_vars
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
        self.env_vars = sanitized_config;
    }
}

pub struct TappBuilder;

impl TappBuilder {
    fn validate_image(deployment_config: Deployment, container: &Container) -> Result<String, TappRequestError> {
        if let Some(allow_list) = deployment_config.allowed_images {
            if allow_list.iter().any(|image| container.image.contains(image)) {
                Ok(container.image.clone())
            } else {
                Err(TappRequestError::Image(format!("{}.\nAllowed images: {:?}", container.image, allow_list)))
            }
        } else {
            Ok(container.image.clone())
        }
    }

    fn validate_name(subdomain: &str, re: &Regex) -> Result<(), TappRequestError> {
        if !re.is_match(subdomain) {
            Err(TappRequestError::Domain(subdomain.to_string()))
        } else {
            Ok(())
        }
    }

    fn validate_custom_domain(custom: &str) -> Result<(), TappRequestError> {
        let re = Regex::new(r"^(?:[a-z0-9]([a-z0-9-]{1,61}[a-z0-9])?\.)?[a-z0-9]([a-z0-9-]{1,61}[a-z0-9])?\.(?:[a-z]{2,}\.)?[a-z]{2,}$").unwrap();
        if !re.is_match(custom) {
            Err(TappRequestError::Domain(custom.to_string()))
        } else {
            Ok(())
        }
    }

    fn create_secrets() -> BTreeMap<String, String> {
        let mut secrets = BTreeMap::new();
        secrets.insert("test-secret".to_owned(), "1234".to_owned());
        secrets
    }

    fn create_labels(req: &TappRequest) -> BTreeMap<String, String> {
        let mut labels = BTreeMap::new();
        labels.insert("owner".to_owned(), req.owner.replace('@', "-"));
        labels.insert("fingerprint".to_owned(), sha1_smol::Sha1::from(&req.owner).digest().to_string());
        labels
    }

}
impl TryFrom<TappRequest> for TailoredApp {
    type Error = TappRequestError;

    fn try_from(mut req: TappRequest) -> Result<TailoredApp, TappRequestError> {
    req.sanitize_input();

    req.container.image = TappBuilder::validate_image(req.kubetailor.deployment.clone(), &req.container)?;
    let name_regex = Regex::new(r"^[a-z0-9]([a-z0-9-]{1,61}[a-z0-9])$").unwrap();
    TappBuilder::validate_name(&req.domains.shared, &name_regex)?;
    TappBuilder::validate_name(&req.name, &name_regex)?;

    if let Some(custom) = &req.domains.custom {
        TappBuilder::validate_custom_domain(custom)?;
    }

    let secrets = TappBuilder::create_secrets();
    let labels = TappBuilder::create_labels(&req);

    let tapp_spec = TailoredAppSpec {
        labels: labels.clone(),
        deployment: req.kubetailor.deployment.build(&req.container),
        ingress: req.kubetailor.ingress.build(req.domains.shared, req.domains.custom),
        env_vars: req.env_vars.clone(),
        secrets,
    };

    Ok(TailoredApp::new(&req.name.to_lowercase(), tapp_spec.clone()))
}
}

impl TryFrom<TailoredApp> for TappRequest {
    type Error = TappRequestError;

    fn try_from(tapp: TailoredApp) -> Result<Self, Self::Error> {
        let app_config = TappRequest {
            name: tapp.metadata.name.unwrap(),
            owner: String::new(),
            domains: tapp.spec.ingress.domains,
            container: tapp.spec.deployment.container,
            env_vars: tapp.spec.env_vars,
            secrets: BTreeMap::new(),
            kubetailor: Kubetailor::default(),
        };

        Ok(app_config)
    }
}
