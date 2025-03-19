use kubetailor::{
    k8s_openapi::api::core::v1::{ContainerState, ContainerStatus},
    prelude::*,
};
use std::convert::TryFrom;

#[derive(Deserialize, Serialize)]
pub struct Health {
    deployment: Deployment,
    domains: Domains,
}

#[derive(Deserialize, Serialize)]
struct Deployment {
    replicas: Replicas,
    restarts: i32,
    ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<ContainerState>,
}

#[derive(Deserialize, Serialize)]
struct Replicas {
    desired: i32,
    ready: i32,
}

#[derive(Deserialize, Serialize)]
struct Domains {
    domains: Vec<String>,
    dns: bool,
    ssl: bool,
}

impl TryFrom<Resources> for Health {
    type Error = &'static str;

    fn try_from(value: Resources) -> Result<Self, Self::Error> {
        let certs = value.cert.unwrap_or_default();
        let pods = value.pods.unwrap_or_default();
        let deployments = value.deployment.unwrap_or_default();
        let ingresses = value.ingress.unwrap_or_default();

        // Assuming the first deployment is the one we're interested in
        let deploy_spec = deployments
            .first()
            .and_then(|deploy| deploy.spec.as_ref())
            .ok_or("Deployment spec not found")?;

        let deploy_status = deployments
            .first()
            .and_then(|deploy| deploy.status.as_ref())
            .ok_or("Deployment status not found")?;

        // Certificate status
        let cert_status = certs
            .first()
            .and_then(|cert| cert.status.as_ref())
            .and_then(|s| s.conditions.first())
            .map(|c| c.status == "True" && c.reason == "Ready")
            .unwrap_or(false);

        // Domain status
        let domains_in_use = ingresses
            .first()
            .and_then(|ing| ing.spec.as_ref())
            .and_then(|spec| spec.tls.as_ref())
            .map(|tls| tls.iter().flat_map(|t| t.hosts.clone()).collect::<Vec<_>>())
            .unwrap_or_default();

        let lb_status = ingresses
            .first()
            .and_then(|ing| ing.status.as_ref())
            .is_some();

        let default_cs = ContainerStatus::default();
        // Extracting the first container status if available, or providing defaults
        let container_status = pods
            .first()
            .and_then(|pod| pod.status.as_ref())
            .and_then(|status| status.container_statuses.as_ref())
            .and_then(|statuses| statuses.first());
        let status = container_status.unwrap_or(&default_cs);

        // Use the extracted status to get restart count, ready status, and state
        let restarts = status.restart_count;

        let ready = status.ready;

        Ok(Health {
            deployment: Deployment {
                replicas: Replicas {
                    desired: deploy_spec.replicas.unwrap_or(0),
                    ready: deploy_status.ready_replicas.unwrap_or(0),
                },
                restarts,
                ready,
                state: status.state.clone(),
            },
            domains: Domains {
                domains: domains_in_use.first().unwrap().clone(),
                dns: lb_status,
                ssl: cert_status,
            },
        })
    }
}
