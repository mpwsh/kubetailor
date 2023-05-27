use kubetailor::crd::{TailoredApp, TailoredAppSpec, Container, Domains};
use regex::Regex;
use std::collections::BTreeMap;
use serde::{Serialize, Deserialize};
use super::error::TappRequestError;

#[derive(Serialize, Deserialize, Debug)]
pub struct TappRequest {
    pub name: String,
    pub owner: String,
    pub domains: Domains,
    pub container: Container,
    #[serde(skip_deserializing, skip_serializing)]
    pub labels: BTreeMap<String, String>,
    pub env_vars: BTreeMap<String, String>,
    #[serde(skip_deserializing, skip_serializing)]
    pub secrets: BTreeMap<String, String>,
    #[serde(skip_deserializing, skip_serializing)]
    pub kubetailor: super::config::Kubetailor,
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

impl TryFrom<TappRequest> for TailoredApp {
    type Error = TappRequestError;

    fn try_from(mut app: TappRequest) -> Result<Self, Self::Error> {
        app.sanitize_input();
        //parse shared subdomain
        let re = Regex::new(r"^[a-z0-9]([a-z0-9-]{1,61}[a-z0-9])$").unwrap();
        if !re.is_match(&app.domains.shared) {
            return Err(TappRequestError::Domain(app.domains.shared));
        };
        if !re.is_match(&app.name) {
            return Err(TappRequestError::Name(app.name));
        };
        if let Some(custom) = app.domains.custom.clone() {
            //parse custom domain
            let re = Regex::new(r"^(?:[a-z0-9]([a-z0-9-]{1,61}[a-z0-9])?\.)?[a-z0-9]([a-z0-9-]{1,61}[a-z0-9])?\.(?:[a-z]{2,}\.)?[a-z]{2,}$").unwrap();
            if !re.is_match(&custom) {
                return Err(TappRequestError::Domain(custom));
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
            deployment: app.kubetailor.deployment.build(&app.container),
            ingress: app
                .kubetailor
                .ingress.build(app.domains.shared, app.domains.custom),
            env_vars: app.env_vars,
            secrets,
        });

        tapp.metadata.labels = Some(labels);
        Ok(tapp)
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
            labels: BTreeMap::new(),
            kubetailor: super::config::Kubetailor::default(),
        };

        Ok(app_config)
    }
}
