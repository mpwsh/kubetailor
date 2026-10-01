//! The wizard posts a plain form (htmx's default encoding). Its fields are flat and repeated —
//! `environment_key`/`environment_value` pairs, `file_key[]` followed by the matching
//! `hidden-file-content-<id>` — so the request is read as an ordered list of pairs and folded
//! into a [`TappConfig`] here, with the validation the UI can report inline.

use std::collections::HashMap;

use crate::models::{Container, Domains, Git, Port, TappConfig};

type Pairs = [(String, String)];

/// First non-empty value of `key`. The wizard renders some inputs twice (the Docker and Git
/// source panes both have `image`/`port`/`replicas`), so "first" is "the one the user filled".
fn first(pairs: &Pairs, key: &str) -> Option<String> {
    pairs
        .iter()
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v.trim())
        .find(|v| !v.is_empty())
        .map(str::to_owned)
}

/// `<prefix>_key` / `<prefix>_value` inputs, zipped in document order.
fn pairs_of(pairs: &Pairs, prefix: &str) -> HashMap<String, String> {
    let key_name = format!("{prefix}_key");
    let value_name = format!("{prefix}_value");
    let keys = pairs.iter().filter(|(k, _)| *k == key_name).map(|(_, v)| v);
    let values = pairs
        .iter()
        .filter(|(k, _)| *k == value_name)
        .map(|(_, v)| v);
    keys.zip(values)
        .filter(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
        .map(|(k, v)| (k.trim().to_owned(), v.clone()))
        .collect()
}

/// `file_key[]` inputs, each followed (in document order) by its `hidden-file-content-<id>`.
fn files_of(pairs: &Pairs) -> HashMap<String, String> {
    let mut files = HashMap::new();
    let mut pending: Option<String> = None;
    for (k, v) in pairs {
        if k == "file_key[]" {
            pending = Some(v.trim().to_owned()).filter(|p| !p.is_empty());
        } else if k.starts_with("hidden-file-content-") {
            if let Some(path) = pending.take() {
                files.insert(path, v.clone());
            }
        }
    }
    files
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

/// Like [`number`], but an absent or empty input is `None` rather than an error.
fn optional_number(pairs: &Pairs, key: &str, min: u32, max: u32) -> Result<Option<u32>, String> {
    first(pairs, key)
        .map(|raw| parse_number(&raw, key, min, max))
        .transpose()
}

/// The ports repeater posts one `port_number` / `port_protocol` / `port_expose` triple per row,
/// in document order. A row whose number is blank is an unfilled "Add port" and is skipped.
fn ports_of(pairs: &Pairs) -> Result<Vec<Port>, String> {
    let column = |name: &str| -> Vec<&str> {
        pairs
            .iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
            .collect()
    };
    let numbers = column("port_number");
    let protocols = column("port_protocol");
    let exposures = column("port_expose");
    if numbers.len() != protocols.len() || numbers.len() != exposures.len() {
        return Err("the ports list is incomplete; reload the page and try again".to_owned());
    }

    let mut ports: Vec<Port> = Vec::new();
    for ((number, protocol), expose) in numbers.into_iter().zip(protocols).zip(exposures) {
        if number.trim().is_empty() {
            continue;
        }
        let port = parse_number(number, "port", 1, 65535)?;
        let protocol = protocol.trim().to_ascii_uppercase();
        if !Port::PROTOCOLS.contains(&protocol.as_str()) {
            return Err(format!(
                "port {port}: protocol must be TCP or UDP, got `{protocol}`"
            ));
        }
        let expose = expose.trim().to_owned();
        if !Port::EXPOSURES.contains(&expose.as_str()) {
            return Err(format!(
                "port {port}: exposure must be one of cluster, node or nodePort, got `{expose}`"
            ));
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
    Ok(ports)
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

/// Folds the posted pairs into a request for the backend, or the first problem found, worded
/// for the person filling the form.
pub fn tapp_from_form(pairs: &Pairs) -> Result<TappConfig, String> {
    let name = first(pairs, "name")
        .ok_or("Name is required")?
        .to_lowercase();
    if !valid_name(&name) {
        return Err(format!(
            "`{name}` is not a valid name: lowercase letters, digits and dashes, 2 to 63 characters"
        ));
    }
    let image = first(pairs, "image").ok_or("Image is required")?;
    let port = optional_number(pairs, "port", 1, 65535)?;
    let ports = ports_of(pairs)?;
    if port.is_none() && ports.is_empty() {
        return Err(
            "The app must listen somewhere: set the HTTP port or add at least one port".to_owned(),
        );
    }
    let replicas = number(pairs, "replicas", 1, 10)?;
    let region = first(pairs, "region").map(|r| r.to_lowercase());

    let shared = first(pairs, "shared").unwrap_or_else(|| name.clone());
    if !valid_name(&shared) {
        return Err(format!(
            "`{shared}` is not a valid subdomain: lowercase letters, digits and dashes"
        ));
    }
    // A disabled input is not submitted, so "custom domain off" simply arrives as no `custom`.
    let custom = first(pairs, "custom");

    let repository = first(pairs, "repository");
    let git = repository.map(|repository| Git {
        repository: Some(repository),
        branch: first(pairs, "branch").or_else(|| Some("main".to_owned())),
    });

    let env = pairs_of(pairs, "environment");
    let secrets = pairs_of(pairs, "secret");
    let volumes = pairs_of(pairs, "volume");
    let files = files_of(pairs);

    Ok(TappConfig {
        name,
        // The API wants a string here; an empty group is what the wizard always sent.
        group: Some(first(pairs, "group").unwrap_or_default()),
        owner: String::new(),
        domains: Domains { custom, shared },
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

    #[test]
    fn folds_the_wizard_fields() {
        let pairs = p(&[
            ("name", "Hello"),
            ("group", "default"),
            ("image", "nginx"),
            ("port", "80"),
            ("replicas", "1"),
            ("image", ""), // the git pane's empty copy
            ("environment_key", "A"),
            ("environment_value", "1"),
            ("environment_key", ""),
            ("environment_value", "ignored"),
            ("volume_key", "/data"),
            ("volume_value", "1Gi"),
            ("file_key[]", "/etc/app/conf.toml"),
            ("hidden-file-content-17", "x = 1"),
            ("shared", "hello"),
        ]);
        let t = tapp_from_form(&pairs).unwrap();
        assert_eq!(t.name, "hello");
        assert_eq!(t.container.image, "nginx");
        assert_eq!(t.container.port, Some(80));
        assert!(t.container.ports.is_empty());
        assert!(t.region.is_none());
        assert_eq!(t.env.unwrap()["A"], "1");
        assert_eq!(t.container.volumes.unwrap()["/data"], "1Gi");
        assert_eq!(t.container.files.unwrap()["/etc/app/conf.toml"], "x = 1");
        assert!(t.domains.custom.is_none());
        assert!(t.git.is_none());
        assert!(t.secrets.is_none());
    }

    #[test]
    fn reports_the_first_problem() {
        let err = tapp_from_form(&p(&[("name", "Hello World"), ("image", "nginx")])).unwrap_err();
        assert!(err.contains("not a valid name"), "{err}");
        let err = tapp_from_form(&p(&[("name", "ok"), ("image", "nginx"), ("port", "99999")]))
            .unwrap_err();
        assert!(err.contains("port"), "{err}");
    }

    #[test]
    fn folds_the_ports_repeater() {
        let t = tapp_from_form(&p(&[
            ("name", "game"),
            ("image", "game:1"),
            ("port", ""),
            ("replicas", "1"),
            ("region", "SCL"),
            ("port_number", "7777"),
            ("port_protocol", "UDP"),
            ("port_expose", "node"),
            ("port_number", ""), // an "Add port" row left blank
            ("port_protocol", "TCP"),
            ("port_expose", "cluster"),
            ("port_number", "9000"),
            ("port_protocol", "tcp"),
            ("port_expose", "nodePort"),
        ]))
        .unwrap();
        assert_eq!(t.container.port, None);
        assert_eq!(t.region.as_deref(), Some("scl"));
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
        assert_eq!(json["container"]["ports"][1]["protocol"], "TCP");
        assert!(json["container"].get("port").is_none());
    }

    #[test]
    fn an_app_must_listen_somewhere() {
        let err = tapp_from_form(&p(&[
            ("name", "quiet"),
            ("image", "x"),
            ("port", ""),
            ("replicas", "1"),
        ]))
        .unwrap_err();
        assert!(err.contains("listen somewhere"), "{err}");

        let t = tapp_from_form(&p(&[
            ("name", "ok"),
            ("image", "x"),
            ("port", "80"),
            ("replicas", "1"),
        ]))
        .unwrap();
        assert_eq!(t.group.as_deref(), Some(""), "the API rejects a null group");

        let err = tapp_from_form(&p(&[
            ("name", "dup"),
            ("image", "x"),
            ("replicas", "1"),
            ("port_number", "53"),
            ("port_protocol", "UDP"),
            ("port_expose", "node"),
            ("port_number", "53"),
            ("port_protocol", "UDP"),
            ("port_expose", "cluster"),
        ]))
        .unwrap_err();
        assert!(err.contains("twice"), "{err}");

        let err = tapp_from_form(&p(&[
            ("name", "bad"),
            ("image", "x"),
            ("replicas", "1"),
            ("port_number", "53"),
            ("port_protocol", "SCTP"),
            ("port_expose", "node"),
        ]))
        .unwrap_err();
        assert!(err.contains("TCP or UDP"), "{err}");
    }

    #[test]
    fn git_source_defaults_branch() {
        let t = tapp_from_form(&p(&[
            ("name", "svc"),
            ("image", "rust:1"),
            ("port", "8080"),
            ("replicas", "1"),
            ("repository", "https://github.com/x/y"),
        ]))
        .unwrap();
        let git = t.git.unwrap();
        assert_eq!(git.branch.as_deref(), Some("main"));
    }
}
