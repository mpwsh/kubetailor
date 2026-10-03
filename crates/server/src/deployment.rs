use std::collections::BTreeMap;

use kubetailor::crd::{self, Container};
use serde::{Deserialize, Serialize};

use crate::quantity::{bytes, cpu_millis, Quantity};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Deployment {
    pub allowed_images: Option<Vec<String>>,
    pub deploy_network_policies: Option<bool>,
    pub enable_service_links: Option<bool>,
    pub service_account: Option<String>,
    pub allow_privilege_escalation: Option<bool>,
    pub allow_root: Option<bool>,
    pub run_as_user: Option<i64>,
    pub run_as_group: Option<i64>,
    pub annotations: BTreeMap<String, String>,
    pub container: Option<Container>,
    /// Inclusive range users may expose at the node (`expose: node` / `nodePort`).
    /// Defaults to 1024-29999: above the privileged ports, below the Kubernetes node-port range.
    #[serde(default)]
    pub node_port_range: Option<(i32, i32)>,
    /// What one app may ask for. Unset bounds are not enforced.
    #[serde(default)]
    pub resources: Limits,
}

/// Bounds on an app's resources, in Kubernetes quantities. The minimum doubles as the default
/// when a request names no `resources`.
#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    #[serde(default)]
    pub cpu: Range,
    #[serde(default)]
    pub memory: Range,
    #[serde(default)]
    pub volume: VolumeLimits,
}

impl Limits {
    /// Every bound is a quantity its field understands, and no minimum exceeds its maximum.
    /// Checked at start-up, so a typo in the config (`128Mib`) stops the server with a clear
    /// message instead of refusing every deployment.
    pub fn validate(&self) -> Result<(), String> {
        self.cpu.validate("deployment.resources.cpu", cpu_millis)?;
        self.memory.validate("deployment.resources.memory", bytes)?;
        self.volume
            .size
            .validate("deployment.resources.volume.size", bytes)?;
        if let (Some(min), Some(max)) = (self.volume.count.min, self.volume.count.max) {
            if min > max {
                return Err(format!(
                    "deployment.resources.volume.count: min {min} is above max {max}"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq, Eq)]
pub struct Range {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<Quantity>,
}

impl Range {
    fn validate(&self, what: &str, parse: fn(&str) -> Option<u64>) -> Result<(), String> {
        let bound = |end: &str, q: &Quantity| {
            parse(q.as_str()).ok_or_else(|| {
                format!(
                    "{what}.{end}: `{}` is not a Kubernetes quantity (e.g. 250m, 0.5, 128Mi, 2Gi)",
                    q.as_str()
                )
            })
        };
        let min = self.min.as_ref().map(|q| bound("min", q)).transpose()?;
        let max = self.max.as_ref().map(|q| bound("max", q)).transpose()?;
        if let (Some(min), Some(max)) = (min, max) {
            if min > max {
                return Err(format!("{what}: min is above max"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq, Eq)]
pub struct VolumeLimits {
    #[serde(default)]
    pub count: CountRange,
    #[serde(default)]
    pub size: Range,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq, Eq)]
pub struct CountRange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<usize>,
}

impl Deployment {
    pub fn build(&self, container: &Container, region: Option<String>) -> crd::Deployment {
        //If theres a container spec in the server configuration, use that instead of the one provided by the user.
        let container = self.container.clone().unwrap_or_else(|| container.clone());
        crd::Deployment {
            annotations: self.annotations.clone(),
            region,
            container,
            service_account: self.service_account.to_owned(),
            allow_privilege_escalation: self.allow_privilege_escalation,
            allow_root: self.allow_root,
            run_as_user: self.run_as_user,
            run_as_group: self.run_as_group,
            enable_service_links: self.enable_service_links,
            deploy_network_policies: self.deploy_network_policies,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_checked_at_start_up() {
        let ok: Limits = serde_yaml::from_str(
            "cpu: {min: 0.2, max: 1}\nmemory: {min: 128Mi, max: 2Gi}\nvolume: {count: {max: 2}, size: {max: 2Gi}}",
        )
        .unwrap();
        assert!(ok.validate().is_ok());
        let typo: Limits = serde_yaml::from_str("memory: {min: 128Mib}").unwrap();
        let err = typo.validate().unwrap_err();
        assert!(
            err.contains("memory.min") && err.contains("128Mib"),
            "{err}"
        );
        let inverted: Limits = serde_yaml::from_str("cpu: {min: 2, max: 1}").unwrap();
        assert!(inverted
            .validate()
            .unwrap_err()
            .contains("min is above max"));
        assert!(Limits::default().validate().is_ok());
    }
}
