use std::collections::BTreeMap;

use kubetailor::crd::{self, Container};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Deployment {
    pub allowed_images: Option<Vec<String>>,
    pub deploy_network_policies: bool,
    pub annotations: BTreeMap<String, String>,
    pub container: Option<Container>,
}

impl Deployment {
    pub fn build(&self, container: &Container) -> crd::Deployment {
        let mut annotations = BTreeMap::new();
        //If theres a container spec in the server configuration, use that instead of the one provided by the user.
        let container = if let Some(container) = self.container.clone() {
            Container {
                image: container.image,
                port: container.port,
                replicas: container.replicas,
                build_command: container.build_command,
                run_command: container.run_command,
                git_repository: None,
                volumes: None,
                file_mounts: None,
            }
        } else {
            container.clone()
        };

        for a in self.annotations.iter() {
            annotations.insert(a.0.to_owned(), a.1.to_owned());
        }
        crd::Deployment {
            annotations,
            container,
        }
    }
}
