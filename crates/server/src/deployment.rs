use std::collections::BTreeMap;

use kubetailor::crd::{self, Container};
use serde::{Deserialize, Serialize};

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
