//! Kubernetes resource quantities, as far as comparing a request against a limit needs: CPU in
//! millicores, memory and storage in bytes. Accepts what people and configs write (`250m`,
//! `0.5`, `2`, `256Mi`, `2Gi`, `1.5G`, `1073741824`) and refuses the rest.

use serde::{Deserialize, Deserializer, Serialize};

/// A quantity as written in a config or a request. Deserialises from a string or a bare YAML
/// number (`max: 1`), since both read naturally in a config file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Quantity(pub String);

impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Text(String),
            Int(u64),
            Float(f64),
        }
        Ok(Quantity(match Raw::deserialize(d)? {
            Raw::Text(s) => s,
            Raw::Int(n) => n.to_string(),
            Raw::Float(f) => f.to_string(),
        }))
    }
}

impl Quantity {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn split(s: &str) -> Option<(f64, &str)> {
    let s = s.trim();
    let end = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(s.len());
    let (digits, suffix) = s.split_at(end);
    if digits.is_empty() || digits.matches('.').count() > 1 {
        return None;
    }
    Some((digits.parse().ok()?, suffix))
}

/// CPU in millicores: `250m` → 250, `0.5` → 500, `2` → 2000.
pub fn cpu_millis(s: &str) -> Option<u64> {
    let (n, suffix) = split(s)?;
    let millis = match suffix {
        "m" => n,
        "" => n * 1000.0,
        _ => return None,
    };
    (millis >= 0.0).then(|| millis.round() as u64)
}

/// Memory or storage in bytes: binary (`Ki`, `Mi`, `Gi`, `Ti`), decimal (`k`, `M`, `G`, `T`)
/// or plain bytes.
pub fn bytes(s: &str) -> Option<u64> {
    let (n, suffix) = split(s)?;
    let unit: f64 = match suffix {
        "" => 1.0,
        "Ki" => 1024.0,
        "Mi" => 1024.0 * 1024.0,
        "Gi" => 1024.0 * 1024.0 * 1024.0,
        "Ti" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        "k" => 1e3,
        "M" => 1e6,
        "G" => 1e9,
        "T" => 1e12,
        _ => return None,
    };
    let total = n * unit;
    (total >= 0.0).then(|| total.round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_spellings() {
        assert_eq!(cpu_millis("250m"), Some(250));
        assert_eq!(cpu_millis("0.5"), Some(500));
        assert_eq!(cpu_millis("1"), Some(1000));
        assert_eq!(cpu_millis(" 2.25 "), Some(2250));
        assert_eq!(cpu_millis("1Gi"), None);
        assert_eq!(cpu_millis("lots"), None);
        assert_eq!(cpu_millis(""), None);
    }

    #[test]
    fn byte_spellings() {
        assert_eq!(bytes("256Mi"), Some(256 << 20));
        assert_eq!(bytes("2Gi"), Some(2 << 30));
        assert_eq!(bytes("1.5Gi"), Some(3 << 29));
        assert_eq!(bytes("500M"), Some(500_000_000));
        assert_eq!(bytes("1024"), Some(1024));
        assert_eq!(bytes("128Mib"), None, "not a Kubernetes unit");
        assert_eq!(bytes("2 Gi"), None);
        assert_eq!(bytes("big"), None);
    }

    #[test]
    fn quantities_read_from_numbers_or_strings() {
        let q: Vec<Quantity> = serde_yaml::from_str("[\"250m\", 1, 0.5]").unwrap();
        assert_eq!(q[0].as_str(), "250m");
        assert_eq!(q[1].as_str(), "1");
        assert_eq!(q[2].as_str(), "0.5");
    }
}
