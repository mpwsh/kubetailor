//! The wizard posts a plain form (htmx's default encoding). Its fields are flat and repeated —
//! `environment_key`/`environment_value` pairs, `file_key[]` followed by the matching
//! `hidden-file-content-<id>` — so the request is read as an ordered list of pairs and folded
//! into a [`TappConfig`] here, with the validation the UI can report inline.

use std::collections::HashMap;

use crate::models::{Container, Domains, Git, TappConfig};

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

fn number(pairs: &Pairs, key: &str, min: u32, max: u32) -> Result<u32, String> {
    let raw = first(pairs, key).ok_or_else(|| format!("{key} is required"))?;
    let n: u32 = raw
        .parse()
        .map_err(|_| format!("{key} must be a whole number, got `{raw}`"))?;
    if n < min || n > max {
        return Err(format!("{key} must be between {min} and {max}"));
    }
    Ok(n)
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
    let port = number(pairs, "port", 1, 65535)?;
    let replicas = number(pairs, "replicas", 1, 10)?;

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
        group: first(pairs, "group"),
        owner: String::new(),
        domains: Domains { custom, shared },
        container: Container {
            image,
            replicas,
            port,
            volumes: (!volumes.is_empty()).then_some(volumes),
            files: (!files.is_empty()).then_some(files),
            build_command: first(pairs, "buildcmd"),
            run_command: first(pairs, "runcmd"),
        },
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
        assert_eq!(t.container.port, 80);
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
