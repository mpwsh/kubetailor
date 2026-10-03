use std::collections::BTreeMap;

use kubetailor::crd::{Container, Domains, Resources, TailoredApp, TailoredAppSpec};
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::{
    config::{Kubetailor, Region},
    deployment::{Deployment, Limits},
    error::TappRequestError,
    git::Git,
    quantity::{bytes, cpu_millis, Quantity},
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TappRequest {
    pub name: String,
    #[serde(skip_serializing)]
    pub owner: String,
    pub group: String,
    pub container: Container,
    pub domains: Option<Domains>,
    /// Region the app must run in (a node label); unset lets the scheduler choose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
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

/// Node-exposed ports default to this inclusive range: above the privileged ports (the ingress
/// controller's host ports live there), below the Kubernetes node-port range.
pub const DEFAULT_NODE_PORT_RANGE: (i32, i32) = (1024, 29999);

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

    /// An app must listen somewhere, and node-exposed ports must stay out of the privileged and
    /// node-port ranges (the former clash with the ingress controller's host ports, the latter
    /// with what Kubernetes allocates).
    fn validate_ports(
        deployment_config: &Deployment,
        container: &Container,
    ) -> Result<(), TappRequestError> {
        if container.port.is_none() && container.ports.is_empty() {
            return Err(TappRequestError::Port(
                "set `port` (HTTP) or at least one entry in `ports`".to_owned(),
            ));
        }
        let (min, max) = deployment_config
            .node_port_range
            .unwrap_or(DEFAULT_NODE_PORT_RANGE);
        for p in container.ports.iter().filter(|p| p.is_external()) {
            if p.port < min || p.port > max {
                return Err(TappRequestError::Port(format!(
                    "port {}/{} exposed at the node must be within {min}-{max}",
                    p.port,
                    p.protocol.as_str()
                )));
            }
        }
        Ok(())
    }

    /// A region must be one the operator offers, when a list is configured.
    fn validate_region(regions: &[Region], region: Option<&str>) -> Result<(), TappRequestError> {
        let Some(region) = region.map(str::trim).filter(|r| !r.is_empty()) else {
            return Ok(());
        };
        if regions.is_empty() || regions.iter().any(|r| r.id == region) {
            return Ok(());
        }
        Err(TappRequestError::Region(format!(
            "`{region}` is not offered; regions: {}",
            regions
                .iter()
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )))
    }

    /// What one replica gets: the request's `resources` kept within the configured bounds, or
    /// the minimums when the request names none (so every pod carries requests and the
    /// scheduler can tell a node is full). `None` only when nothing is configured either.
    fn resolve_resources(
        limits: &Limits,
        requested: Option<&Resources>,
    ) -> Result<Option<Resources>, TappRequestError> {
        let bound = |what: &str, value: &Quantity, parse: fn(&str) -> Option<u64>| {
            parse(value.as_str()).ok_or_else(|| {
                TappRequestError::Resources(format!(
                    "configured {what} bound `{}` is not a quantity",
                    value.as_str()
                ))
            })
        };
        let check = |what: &str,
                     value: &str,
                     parse: fn(&str) -> Option<u64>,
                     min: Option<&Quantity>,
                     max: Option<&Quantity>|
         -> Result<(), TappRequestError> {
            let n = parse(value).ok_or_else(|| {
                TappRequestError::Resources(format!("{what} `{value}` is not a quantity"))
            })?;
            if let Some(min) = min {
                if n < bound(what, min, parse)? {
                    return Err(TappRequestError::Resources(format!(
                        "{what} {value} is below the minimum {}",
                        min.as_str()
                    )));
                }
            }
            if let Some(max) = max {
                if n > bound(what, max, parse)? {
                    return Err(TappRequestError::Resources(format!(
                        "{what} {value} is above the maximum {}",
                        max.as_str()
                    )));
                }
            }
            Ok(())
        };
        let requested = match requested {
            Some(r) => r.clone(),
            None => match (&limits.cpu.min, &limits.memory.min) {
                (Some(cpu), Some(memory)) => Resources {
                    cpu: cpu.as_str().to_owned(),
                    memory: memory.as_str().to_owned(),
                },
                _ => return Ok(None),
            },
        };
        check(
            "cpu",
            &requested.cpu,
            cpu_millis,
            limits.cpu.min.as_ref(),
            limits.cpu.max.as_ref(),
        )?;
        check(
            "memory",
            &requested.memory,
            bytes,
            limits.memory.min.as_ref(),
            limits.memory.max.as_ref(),
        )?;
        Ok(Some(requested))
    }

    /// Volumes within the configured count and size bounds.
    fn validate_volumes(limits: &Limits, container: &Container) -> Result<(), TappRequestError> {
        let volumes = container.volumes.as_ref();
        let count = volumes.map(BTreeMap::len).unwrap_or(0);
        if let Some(max) = limits.volume.count.max {
            if count > max {
                return Err(TappRequestError::Volume(format!(
                    "{count} volumes; at most {max} per deployment"
                )));
            }
        }
        let parse_bound = |b: &Quantity| {
            bytes(b.as_str()).ok_or_else(|| {
                TappRequestError::Volume(format!(
                    "configured size bound `{}` is not a quantity",
                    b.as_str()
                ))
            })
        };
        for (path, size) in volumes.into_iter().flatten() {
            let n = bytes(size).ok_or_else(|| {
                TappRequestError::Volume(format!("`{path}`: size `{size}` is not a quantity"))
            })?;
            if let Some(min) = &limits.volume.size.min {
                if n < parse_bound(min)? {
                    return Err(TappRequestError::Volume(format!(
                        "`{path}`: {size} is below the minimum {}",
                        min.as_str()
                    )));
                }
            }
            if let Some(max) = &limits.volume.size.max {
                if n > parse_bound(max)? {
                    return Err(TappRequestError::Volume(format!(
                        "`{path}`: {size} is above the maximum {}",
                        max.as_str()
                    )));
                }
            }
        }
        Ok(())
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
                }
                _ => None,
            }
        });

        TappBuilder::validate_name(&req.name, &name_regex)?;
        TappBuilder::validate_ports(&req.kubetailor.deployment, &req.container)?;
        TappBuilder::validate_region(&req.kubetailor.regions, req.region.as_deref())?;
        TappBuilder::validate_volumes(&req.kubetailor.deployment.resources, &req.container)?;
        req.container.resources = TappBuilder::resolve_resources(
            &req.kubetailor.deployment.resources,
            req.container.resources.as_ref(),
        )?;
        let labels = TappBuilder::create_labels(&req);
        let tapp_spec = TailoredAppSpec {
            labels: labels.clone(),
            deployment: req
                .kubetailor
                .deployment
                .build(&req.container, req.region.clone()),
            git,
            ingress: Some(req.kubetailor.ingress.build(req.domains)),
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
        // The group only lives in the labels. A read that dropped it made every edit save the
        // app with an empty group, which changed its labels (and, before the operator stopped
        // selecting pods on them, broke its Deployment).
        let group = tapp
            .metadata
            .labels
            .as_ref()
            .and_then(|l| l.get("group"))
            .cloned()
            .unwrap_or_default();
        let app_config = TappRequest {
            name: tapp.metadata.name.unwrap(),
            owner: String::new(),
            group,
            git: git_option.map(|git| Git {
                repository: git.repository.clone(),
                branch: git.branch.clone(),
                username: git.username.clone(),
                token: git.token.clone(),
                period: git.period.clone(),
                image: git.image.clone(),
            }),
            container: tapp.spec.deployment.container,
            domains: tapp.spec.ingress.and_then(|i| i.domains),
            region: tapp.spec.deployment.region,
            env: tapp.spec.env,
            secrets: tapp.spec.secrets,
            kubetailor: Kubetailor::default(),
        };

        Ok(app_config)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use kubetailor::crd::{Container, Deployment as CrdDeployment, TailoredAppSpec};

    use super::*;

    #[test]
    fn reading_an_app_back_keeps_its_group() {
        let mut app = TailoredApp::new(
            "game",
            TailoredAppSpec {
                labels: BTreeMap::new(),
                deployment: CrdDeployment {
                    annotations: BTreeMap::new(),
                    enable_service_links: None,
                    service_account: None,
                    allow_privilege_escalation: None,
                    allow_root: None,
                    run_as_user: None,
                    run_as_group: None,
                    deploy_network_policies: None,
                    region: None,
                    container: Container {
                        image: "game:1".into(),
                        port: None,
                        ports: vec![],
                        run_command: None,
                        build_command: None,
                        volumes: None,
                        files: None,
                        replicas: 1,
                        resources: None,
                    },
                },
                ingress: None,
                env: None,
                secrets: None,
                git: None,
            },
        );
        app.metadata.labels = Some(BTreeMap::from([
            ("owner".to_string(), "x-y.z".to_string()),
            ("group".to_string(), "games".to_string()),
        ]));
        let req = TappRequest::try_from(app).unwrap();
        assert_eq!(req.group, "games");
    }

    fn limits(yaml: &str) -> Limits {
        serde_yaml::from_str(yaml).unwrap()
    }

    fn container(volumes: &[(&str, &str)], resources: Option<(&str, &str)>) -> Container {
        Container {
            image: "x".into(),
            replicas: 1,
            volumes: (!volumes.is_empty()).then(|| {
                volumes
                    .iter()
                    .map(|(p, s)| (p.to_string(), s.to_string()))
                    .collect()
            }),
            resources: resources.map(|(cpu, memory)| Resources {
                cpu: cpu.into(),
                memory: memory.into(),
            }),
            ..Container::default()
        }
    }

    #[test]
    fn resources_default_to_the_minimum_and_stay_under_the_maximum() {
        let l = limits("cpu: {min: 200m, max: 1}\nmemory: {min: 128Mi, max: 2Gi}");
        let defaulted = TappBuilder::resolve_resources(&l, None).unwrap().unwrap();
        assert_eq!(
            (defaulted.cpu.as_str(), defaulted.memory.as_str()),
            ("200m", "128Mi")
        );
        let ok = Resources {
            cpu: "0.5".into(),
            memory: "1Gi".into(),
        };
        assert_eq!(
            TappBuilder::resolve_resources(&l, Some(&ok)).unwrap(),
            Some(ok)
        );
        let greedy = Resources {
            cpu: "4".into(),
            memory: "1Gi".into(),
        };
        let err = TappBuilder::resolve_resources(&l, Some(&greedy)).unwrap_err();
        assert!(err.to_string().contains("above the maximum 1"), "{err}");
        let tiny = Resources {
            cpu: "250m".into(),
            memory: "64Mi".into(),
        };
        let err = TappBuilder::resolve_resources(&l, Some(&tiny)).unwrap_err();
        assert!(err.to_string().contains("below the minimum 128Mi"), "{err}");
        let garbage = Resources {
            cpu: "fast".into(),
            memory: "64Mi".into(),
        };
        assert!(TappBuilder::resolve_resources(&l, Some(&garbage)).is_err());
        // Nothing configured: nothing imposed.
        assert_eq!(
            TappBuilder::resolve_resources(&Limits::default(), None).unwrap(),
            None
        );
    }

    #[test]
    fn volumes_are_bounded_in_size_and_count() {
        let l = limits("volume: {count: {max: 2}, size: {min: 100Mi, max: 2Gi}}");
        assert!(TappBuilder::validate_volumes(&l, &container(&[("/data", "2Gi")], None)).is_ok());
        let err =
            TappBuilder::validate_volumes(&l, &container(&[("/data", "10Gi")], None)).unwrap_err();
        assert!(err.to_string().contains("above the maximum 2Gi"), "{err}");
        let err =
            TappBuilder::validate_volumes(&l, &container(&[("/data", "1Mi")], None)).unwrap_err();
        assert!(err.to_string().contains("below the minimum"), "{err}");
        let err = TappBuilder::validate_volumes(
            &l,
            &container(&[("/a", "1Gi"), ("/b", "1Gi"), ("/c", "1Gi")], None),
        )
        .unwrap_err();
        assert!(err.to_string().contains("at most 2"), "{err}");
        assert!(TappBuilder::validate_volumes(
            &Limits::default(),
            &container(&[("/x", "9Ti")], None)
        )
        .is_ok());
    }

    #[test]
    fn regions_are_checked_against_the_list_when_there_is_one() {
        let regions = vec![
            Region {
                id: "scl".into(),
                name: "Santiago".into(),
            },
            Region {
                id: "waw".into(),
                name: "Warsaw".into(),
            },
        ];
        assert!(TappBuilder::validate_region(&regions, Some("waw")).is_ok());
        assert!(TappBuilder::validate_region(&regions, None).is_ok());
        assert!(TappBuilder::validate_region(&regions, Some("")).is_ok());
        let err = TappBuilder::validate_region(&regions, Some("mars")).unwrap_err();
        assert!(err.to_string().contains("scl, waw"), "{err}");
        assert!(TappBuilder::validate_region(&[], Some("mars")).is_ok());
    }
}
