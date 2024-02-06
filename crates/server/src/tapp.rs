use std::collections::BTreeMap;

use kubetailor::crd::{Container, Domains, TailoredApp, TailoredAppSpec};
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::{config::Kubetailor, deployment::Deployment, error::TappRequestError, git::Git};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TappRequest {
    pub name: String,
    #[serde(skip_serializing)]
    pub owner: String,
    pub group: String,
    pub container: Container,
    pub domains: Option<Domains>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<Git>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<BTreeMap<String, String>>,
    #[serde(skip_deserializing, skip_serializing)]
    pub kubetailor: Kubetailor,
}

impl TappRequest {
    pub fn sanitize_input(&mut self) {
        let sanitized_config: Option<BTreeMap<String, String>> = self.env.as_ref().map(|env| {
            env.iter()
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

                    (sanitized_key, value.clone())
                })
                .collect()
        });
        self.env = sanitized_config;
    }
}

pub struct TappBuilder;

impl TappBuilder {
    fn validate_image(
        deployment_config: Deployment,
        container: &Container,
    ) -> Result<String, TappRequestError> {
        if let Some(allow_list) = deployment_config.allowed_images {
            if allow_list
                .iter()
                .any(|image| container.image.contains(image))
            {
                Ok(container.image.clone())
            } else {
                Err(TappRequestError::Image(format!(
                    "{}.\nAllowed images: {:?}",
                    container.image, allow_list
                )))
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

    fn create_labels(req: &TappRequest) -> BTreeMap<String, String> {
        let mut labels = BTreeMap::new();
        labels.insert("owner".to_owned(), req.owner.replace('@', "-"));
        labels.insert("group".to_owned(), req.group.to_owned());
        labels.insert(
            "fingerprint".to_owned(),
            sha1_smol::Sha1::from(format!("{}{}", req.group, req.owner))
                .digest()
                .to_string(),
        );
        labels
    }
}
impl TryFrom<TappRequest> for TailoredApp {
    type Error = TappRequestError;

    fn try_from(mut req: TappRequest) -> Result<TailoredApp, TappRequestError> {
        req.sanitize_input();

        req.container.image =
            TappBuilder::validate_image(req.kubetailor.deployment.clone(), &req.container)?;
        let name_regex = Regex::new(r"^[a-z0-9]([a-z0-9-]{1,61}[a-z0-9])$").unwrap();

        if let Some(domains) = &req.domains {
            TappBuilder::validate_name(&domains.shared, &name_regex)?;
            if let Some(custom) = &domains.custom {
                TappBuilder::validate_custom_domain(custom)?;
            }
        }

        let git = req.git.as_ref().and_then(|g| {
            let repo = g.repository.clone().filter(|r| !r.is_empty());
            let branch = g.branch.clone();
            let token = g.token.clone().filter(|t| !t.is_empty());
            let username = g.username.clone().filter(|t| !t.is_empty());

            match (repo, branch) {
                (Some(repo), Some(branch)) => {
                    req.kubetailor
                        .git_sync
                        .build(Some(repo), Some(branch), username, token)
                },
                _ => None,
            }
        });

        TappBuilder::validate_name(&req.name, &name_regex)?;
        let labels = TappBuilder::create_labels(&req);
        let tapp_spec = TailoredAppSpec {
            labels: labels.clone(),
            deployment: req.kubetailor.deployment.build(&req.container),
            git,
            ingress: req.kubetailor.ingress.build(req.domains),
            env: req.env.clone(),
            secrets: req.secrets,
        };
        let mut tapp = TailoredApp::new(&req.name.to_lowercase(), tapp_spec);
        tapp.metadata.labels = Some(labels);
        Ok(tapp)
    }
}

impl TryFrom<TailoredApp> for TappRequest {
    type Error = TappRequestError;

    fn try_from(tapp: TailoredApp) -> Result<Self, Self::Error> {
        let git_option = tapp.spec.git.as_ref();
        let app_config = TappRequest {
            name: tapp.metadata.name.unwrap(),
            owner: String::new(),
            group: String::new(),
            git: git_option.map(|git| Git {
                repository: git.repository.clone(),
                branch: git.branch.clone(),
                username: git.username.clone(),
                token: git.token.clone(),
                period: git.period.clone(),
                image: git.image.clone(),
            }),
            container: tapp.spec.deployment.container,
            domains: tapp.spec.ingress.domains,
            env: tapp.spec.env,
            secrets: tapp.spec.secrets,
            kubetailor: Kubetailor::default(),
        };

        Ok(app_config)
    }
}
