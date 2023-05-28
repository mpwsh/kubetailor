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
    pub annotations: BTreeMap<String, String>,
    pub container: Option<Container>,
}

impl Deployment {
    pub fn build(&self, container: &Container) -> crd::Deployment {
        //If theres a container spec in the server configuration, use that instead of the one provided by the user.
        let container = if let Some(container) = self.container.clone() {
            Container {
                image: container.image,
                port: container.port,
                replicas: container.replicas,
                build_command: container.build_command,
                run_command: container.run_command,
                git_repository: None,
                volumes: container.volumes,
                file_mounts: container.file_mounts,
            }
        } else {
            container.clone()
        };
        crd::Deployment {
            annotations: self.annotations.clone(),
            container,
            service_account: self.service_account.to_owned(),
            allow_privilege_escalation: self.allow_privilege_escalation,
            enable_service_links: self.enable_service_links,
            deploy_network_policies: self.deploy_network_policies,
        }
    }
}
