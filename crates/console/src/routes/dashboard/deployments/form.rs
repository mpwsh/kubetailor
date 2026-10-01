//! The wizard posts a plain form (htmx's default encoding). Its fields are flat and repeated —
//! every repeater row posts the same names again (`env_key`/`env_value`, `port_number`/
//! `port_protocol`/`port_expose`, …) — so the request is read as an ordered list of pairs and
//! folded into a [`TappConfig`] here, with the validation the UI can report inline.

use std::collections::HashMap;

use serde::Serialize;

use crate::models::{ApiConfig, Container, Domains, Git, Port, TappConfig};

type Pairs = [(String, String)];

/// Rows the Network step may hold, HTTP included.
pub const MAX_PORTS: usize = 5;

/// First non-empty value of `key`.
fn first(pairs: &Pairs, key: &str) -> Option<String> {
    pairs
        .iter()
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v.trim())
        .find(|v| !v.is_empty())
        .map(str::to_owned)
}

/// Every value posted under `key`, in document order, untrimmed.
fn column<'a>(pairs: &'a Pairs, key: &str) -> Vec<&'a str> {
    pairs
        .iter()
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .collect()
}

/// A two-column repeater (`<prefix>_key` / `<prefix>_value`), zipped in document order. Rows
/// with an empty key are unfilled "Add" rows and are skipped; values keep their whitespace,
/// since a file's content or an env value may need it.
fn pairs_of(pairs: &Pairs, prefix: &str) -> Result<HashMap<String, String>, String> {
    let keys = column(pairs, &format!("{prefix}_key"));
    let values = column(pairs, &format!("{prefix}_value"));
    if keys.len() != values.len() {
        return Err(format!(
            "the {prefix} list is incomplete; reload the page and try again"
        ));
    }
    let mut out = HashMap::new();
    for (k, v) in keys.into_iter().zip(values) {
        let k = k.trim();
        if k.is_empty() {
            continue;
        }
        if out.insert(k.to_owned(), v.to_owned()).is_some() {
            return Err(format!("{prefix} `{k}` is listed twice"));
        }
    }
    Ok(out)
}

fn parse_number(raw: &str, what: &str, min: u32, max: u32) -> Result<u32, String> {
    let n: u32 = raw
        .trim()
        .parse()
        .map_err(|_| format!("{what} must be a whole number, got `{raw}`"))?;
    if n < min || n > max {
        return Err(format!("{what} must be between {min} and {max}"));
    }
    Ok(n)
}

fn number(pairs: &Pairs, key: &str, min: u32, max: u32) -> Result<u32, String> {
    let raw = first(pairs, key).ok_or_else(|| format!("{key} is required"))?;
    parse_number(&raw, key, min, max)
}

fn valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (2..=63).contains(&bytes.len())
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

/// A volume size as Kubernetes spells it: a whole number of Mi or Gi.
fn valid_size(size: &str) -> bool {
    let digits = size.trim_end_matches("Mi").trim_end_matches("Gi");
    digits.len() < size.len() && !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// One row of the Network step as the template seeds and posts it. `HTTP` is the row that
/// becomes [`Container::port`]: reached through the app's domains, so it has no exposure of
/// its own. Everything else becomes an entry of [`Container::ports`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PortRow {
    pub port: String,
    pub protocol: String,
    pub expose: String,
}

/// The rows to seed the Network step with for an existing app (HTTP first), or the one default
/// row (HTTP on 80) for a new one.
pub fn port_rows(container: Option<&Container>) -> Vec<PortRow> {
    let Some(c) = container else {
        return vec![PortRow {
            port: "80".into(),
            protocol: "HTTP".into(),
            expose: "cluster".into(),
        }];
    };
    let http = c.port.map(|p| PortRow {
        port: p.to_string(),
        protocol: "HTTP".into(),
        expose: "cluster".into(),
    });
    http.into_iter()
        .chain(c.ports.iter().map(|p| PortRow {
            port: p.port.to_string(),
            protocol: p.protocol.clone(),
            expose: p.expose.clone(),
        }))
        .collect()
}

/// Folds the Network step back: the HTTP row (at most one) and the others. A row left without
/// a number is ignored.
fn ports_of(pairs: &Pairs, config: &ApiConfig) -> Result<(Option<u32>, Vec<Port>), String> {
    let numbers = column(pairs, "port_number");
    let protocols = column(pairs, "port_protocol");
    let exposures = column(pairs, "port_expose");
    if numbers.len() != protocols.len() || numbers.len() != exposures.len() {
        return Err("the ports list is incomplete; reload the page and try again".to_owned());
    }
    if numbers.len() > MAX_PORTS {
        return Err(format!("at most {MAX_PORTS} ports per deployment"));
    }

    let mut http: Option<u32> = None;
    let mut ports: Vec<Port> = Vec::new();
    for ((number, protocol), expose) in numbers.into_iter().zip(protocols).zip(exposures) {
        if number.trim().is_empty() {
            continue;
        }
        let port = parse_number(number, "port", 1, 65535)?;
        let protocol = protocol.trim().to_ascii_uppercase();
        if protocol == "HTTP" {
            if http.replace(port).is_some() {
                return Err("only one HTTP port: it is the one your domains route to".to_owned());
            }
            continue;
        }
        if !Port::PROTOCOLS.contains(&protocol.as_str()) {
            return Err(format!(
                "port {port}: protocol must be HTTP, TCP or UDP, got `{protocol}`"
            ));
        }
        let expose = expose.trim().to_owned();
        if !Port::EXPOSURES.contains(&expose.as_str()) {
            return Err(format!(
                "port {port}: exposure must be one of cluster, node or nodePort, got `{expose}`"
            ));
        }
        if expose != "cluster" {
            let (min, max) = config.node_port_range;
            if port < min || port > max {
                return Err(format!(
                    "port {port}/{protocol}: ports exposed at the node must be within {min}-{max}"
                ));
            }
        }
        if ports
            .iter()
            .any(|p| p.port == port && p.protocol == protocol)
        {
            return Err(format!("port {port}/{protocol} is listed twice"));
        }
        ports.push(Port {
            port,
            protocol,
            expose,
        });
    }
    Ok((http, ports))
}

/// Folds the posted pairs into a request for the backend, or the first problem found, worded
/// for the person filling the form.
pub fn tapp_from_form(pairs: &Pairs, config: &ApiConfig) -> Result<TappConfig, String> {
    // Basic
    let name = first(pairs, "name")
        .ok_or("Name is required")?
        .to_lowercase();
    if !valid_name(&name) {
        return Err(format!(
            "`{name}` is not a valid name: lowercase letters, digits and dashes, 2 to 63 characters"
        ));
    }
    let region = first(pairs, "region").map(|r| r.to_lowercase());

    // Source
    let image = first(pairs, "image").ok_or("Image is required")?;
    if let Some(allowed) = &config.allowed_images {
        if !allowed.iter().any(|a| image.contains(a)) {
            return Err(format!(
                "`{image}` is not an allowed image; allowed: {}",
                allowed.join(", ")
            ));
        }
    }
    let replicas = number(pairs, "replicas", 1, 10)?;
    let repository = first(pairs, "repository");
    let git = repository.map(|repository| Git {
        repository: Some(repository),
        branch: first(pairs, "branch").or_else(|| Some("main".to_owned())),
    });

    // Network
    let (port, ports) = ports_of(pairs, config)?;
    // The API wants a string here; an empty group is what the wizard always sent.
    let group = first(pairs, "group").unwrap_or_default().to_lowercase();

    // Domains: only meaningful with an HTTP port, which in turn needs one to be reachable.
    let shared = first(pairs, "shared").map(|s| s.to_lowercase());
    let custom = first(pairs, "custom").map(|c| c.to_lowercase());
    let domains = match (shared, port) {
        (Some(shared), Some(_)) => {
            if !valid_name(&shared) {
                return Err(format!(
                    "`{shared}` is not a valid subdomain: lowercase letters, digits and dashes"
                ));
            }
            Some(Domains { custom, shared })
        }
        (Some(_), None) => {
            return Err(
                "A domain needs an HTTP port to route to: add one in the Network step, \
                        or remove the domain"
                    .to_owned(),
            );
        }
        (None, Some(_)) => {
            return Err(
                "The HTTP port is reached through a domain: set one in the Domains \
                        step, or make the port TCP and expose it at the node"
                    .to_owned(),
            );
        }
        (None, None) => {
            if custom.is_some() {
                return Err("A custom domain needs a shared subdomain and an HTTP port".to_owned());
            }
            None
        }
    };
    if port.is_none() && ports.is_empty() {
        return Err("The app must listen somewhere: add a port in the Network step".to_owned());
    }

    // Data
    let volumes = pairs_of(pairs, "volume")?;
    for (path, size) in &volumes {
        if !path.starts_with('/') {
            return Err(format!("volume mount path `{path}` must be absolute"));
        }
        if !valid_size(size) {
            return Err(format!(
                "volume `{path}`: size must be a whole number of Mi or Gi (e.g. 512Mi, 2Gi), got `{size}`"
            ));
        }
    }
    let files = pairs_of(pairs, "file")?;
    for path in files.keys() {
        if !path.starts_with('/') {
            return Err(format!("file path `{path}` must be absolute"));
        }
    }

    // Environment
    let env = pairs_of(pairs, "env")?;
    let secrets = pairs_of(pairs, "secret")?;

    Ok(TappConfig {
        name,
        group: Some(group),
        owner: String::new(),
        domains,
        container: Container {
            image,
            replicas,
            port,
            ports,
            volumes: (!volumes.is_empty()).then_some(volumes),
            files: (!files.is_empty()).then_some(files),
            build_command: first(pairs, "buildcmd"),
            run_command: first(pairs, "runcmd"),
        },
        region,
        git,
        env: (!env.is_empty()).then_some(env),
        secrets: (!secrets.is_empty()).then_some(secrets),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(items: &[(&str, &str)]) -> Vec<(String, String)> {
        items
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn cfg() -> ApiConfig {
        ApiConfig {
            base_domain: "apps.example.com".into(),
            ..ApiConfig::default()
        }
    }

    /// A typical web app: HTTP on 80, a shared subdomain, a few extras.
    fn web() -> Vec<(String, String)> {
        p(&[
            ("name", "Hello"),
            ("region", ""),
            ("image", "nginx"),
            ("replicas", "1"),
            ("port_number", "80"),
            ("port_protocol", "HTTP"),
            ("port_expose", "cluster"),
            ("group", "Web"),
            ("shared", "hello"),
            ("custom", ""),
            ("volume_key", "/data"),
            ("volume_value", "1Gi"),
            ("file_key", "/etc/app/conf.toml"),
            ("file_value", "x = 1\n"),
            ("env_key", "A"),
            ("env_value", "1"),
            ("env_key", ""), // an "Add" row left blank
            ("env_value", "ignored"),
        ])
    }

    #[test]
    fn folds_the_wizard_fields() {
        let t = tapp_from_form(&web(), &cfg()).unwrap();
        assert_eq!(t.name, "hello");
        assert_eq!(t.group.as_deref(), Some("web"));
        assert_eq!(t.container.image, "nginx");
        assert_eq!(t.container.port, Some(80));
        assert!(t.container.ports.is_empty());
        assert!(t.region.is_none());
        let domains = t.domains.unwrap();
        assert_eq!(domains.shared, "hello");
        assert!(domains.custom.is_none());
        assert_eq!(t.env.unwrap()["A"], "1");
        assert_eq!(t.container.volumes.unwrap()["/data"], "1Gi");
        assert_eq!(t.container.files.unwrap()["/etc/app/conf.toml"], "x = 1\n");
        assert!(t.git.is_none());
        assert!(t.secrets.is_none());
    }

    #[test]
    fn a_udp_server_needs_no_domain() {
        let t = tapp_from_form(
            &p(&[
                ("name", "game"),
                ("region", "SCL"),
                ("image", "game:1"),
                ("replicas", "1"),
                ("port_number", "7777"),
                ("port_protocol", "UDP"),
                ("port_expose", "node"),
                ("port_number", ""), // blank row
                ("port_protocol", "TCP"),
                ("port_expose", "cluster"),
                ("port_number", "9000"),
                ("port_protocol", "tcp"),
                ("port_expose", "nodePort"),
                ("group", ""),
                ("shared", ""),
                ("custom", ""),
            ]),
            &cfg(),
        )
        .unwrap();
        assert_eq!(t.container.port, None);
        assert!(t.domains.is_none());
        assert_eq!(t.region.as_deref(), Some("scl"));
        assert_eq!(t.group.as_deref(), Some(""), "the API rejects a null group");
        assert_eq!(
            t.container.ports,
            vec![
                Port {
                    port: 7777,
                    protocol: "UDP".into(),
                    expose: "node".into()
                },
                Port {
                    port: 9000,
                    protocol: "TCP".into(),
                    expose: "nodePort".into()
                },
            ]
        );
        // The API spelling survives the round trip.
        let json = serde_json::to_value(&t).unwrap();
        assert_eq!(json["container"]["ports"][0]["expose"], "node");
        assert!(json["container"].get("port").is_none());
        assert!(json.get("domains").is_none());
    }

    #[test]
    fn http_and_domains_go_together() {
        let mut no_domain = web();
        no_domain.retain(|(k, _)| k != "shared");
        let err = tapp_from_form(&no_domain, &cfg()).unwrap_err();
        assert!(err.contains("reached through a domain"), "{err}");

        let mut no_http = web();
        no_http.retain(|(k, _)| !k.starts_with("port_"));
        let err = tapp_from_form(&no_http, &cfg()).unwrap_err();
        assert!(err.contains("needs an HTTP port"), "{err}");

        let mut two_http = web();
        two_http.extend(p(&[
            ("port_number", "8080"),
            ("port_protocol", "HTTP"),
            ("port_expose", "cluster"),
        ]));
        let err = tapp_from_form(&two_http, &cfg()).unwrap_err();
        assert!(err.contains("only one HTTP port"), "{err}");
    }

    #[test]
    fn ports_are_bounded() {
        let mut many = web();
        for i in 0..5 {
            many.extend(p(&[
                ("port_number", &(2000 + i).to_string()),
                ("port_protocol", "TCP"),
                ("port_expose", "cluster"),
            ]));
        }
        let err = tapp_from_form(&many, &cfg()).unwrap_err();
        assert!(err.contains("at most 5 ports"), "{err}");

        let mut privileged = web();
        privileged.extend(p(&[
            ("port_number", "53"),
            ("port_protocol", "UDP"),
            ("port_expose", "node"),
        ]));
        let err = tapp_from_form(&privileged, &cfg()).unwrap_err();
        assert!(err.contains("1024-29999"), "{err}");

        let mut dup = web();
        dup.extend(p(&[
            ("port_number", "5000"),
            ("port_protocol", "UDP"),
            ("port_expose", "node"),
            ("port_number", "5000"),
            ("port_protocol", "UDP"),
            ("port_expose", "cluster"),
        ]));
        let err = tapp_from_form(&dup, &cfg()).unwrap_err();
        assert!(err.contains("twice"), "{err}");

        let mut sctp = web();
        sctp.extend(p(&[
            ("port_number", "5000"),
            ("port_protocol", "SCTP"),
            ("port_expose", "node"),
        ]));
        let err = tapp_from_form(&sctp, &cfg()).unwrap_err();
        assert!(err.contains("HTTP, TCP or UDP"), "{err}");
    }

    #[test]
    fn reports_data_problems() {
        let mut bad_size = web();
        bad_size.extend(p(&[("volume_key", "/x"), ("volume_value", "big")]));
        let err = tapp_from_form(&bad_size, &cfg()).unwrap_err();
        assert!(err.contains("Mi or Gi"), "{err}");

        let mut relative = web();
        relative.extend(p(&[("file_key", "conf.toml"), ("file_value", "")]));
        let err = tapp_from_form(&relative, &cfg()).unwrap_err();
        assert!(err.contains("must be absolute"), "{err}");

        let mut twice = web();
        twice.extend(p(&[("env_key", "A"), ("env_value", "2")]));
        let err = tapp_from_form(&twice, &cfg()).unwrap_err();
        assert!(err.contains("listed twice"), "{err}");
    }

    #[test]
    fn honours_the_image_allow_list() {
        let config = ApiConfig {
            allowed_images: Some(vec!["postgres".into()]),
            ..cfg()
        };
        let err = tapp_from_form(&web(), &config).unwrap_err();
        assert!(err.contains("not an allowed image"), "{err}");
    }

    #[test]
    fn reports_the_first_problem() {
        let err =
            tapp_from_form(&p(&[("name", "Hello World"), ("image", "nginx")]), &cfg()).unwrap_err();
        assert!(err.contains("not a valid name"), "{err}");
    }

    #[test]
    fn git_source_defaults_branch() {
        let mut form = web();
        form.push(("repository".into(), "https://github.com/x/y".into()));
        let t = tapp_from_form(&form, &cfg()).unwrap();
        assert_eq!(t.git.unwrap().branch.as_deref(), Some("main"));
    }

    #[test]
    fn port_rows_seed_http_first() {
        assert_eq!(port_rows(None)[0].protocol, "HTTP");
        let c = Container {
            port: Some(3000),
            ports: vec![Port {
                port: 7777,
                protocol: "UDP".into(),
                expose: "node".into(),
            }],
            ..Container::default()
        };
        let rows = port_rows(Some(&c));
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (rows[0].port.as_str(), rows[0].protocol.as_str()),
            ("3000", "HTTP")
        );
        assert_eq!(
            (rows[1].port.as_str(), rows[1].expose.as_str()),
            ("7777", "node")
        );
        let c = Container { port: None, ..c };
        assert_eq!(port_rows(Some(&c)).len(), 1);
    }
}
