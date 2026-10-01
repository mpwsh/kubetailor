use kubetailor::{
    crd::Expose,
    k8s_openapi::api::core::v1::{ServicePort, ServiceSpec},
};

use crate::prelude::*;

/// Suffix of the second Service an app gets when it has `expose: nodePort` ports.
pub const NODE_SERVICE_SUFFIX: &str = "-node";

fn service_port(name: String, port: i32, protocol: &str) -> ServicePort {
    ServicePort {
        name: Some(name),
        port,
        target_port: Some(IntOrString::Int(port)),
        protocol: Some(protocol.to_owned()),
        ..ServicePort::default()
    }
}

fn metadata(meta: &TappMeta, name: String) -> ObjectMeta {
    ObjectMeta {
        name: Some(name),
        namespace: Some(meta.namespace.to_owned()),
        labels: Some(meta.labels.to_owned()),
        owner_references: Some(vec![meta.oref.to_owned()]),
        ..ObjectMeta::default()
    }
}

/// The in-cluster Service: the HTTP port plus every `cluster` and `node` port. `None` when the
/// app has nothing to put in it.
fn cluster_service(meta: &TappMeta, app: &TailoredApp) -> Option<Service> {
    let container = &app.spec.deployment.container;
    let mut ports = Vec::new();
    if let Some(http) = container.port {
        // Keep the historical name so existing Ingress backends keep resolving.
        ports.push(service_port(meta.name.to_owned(), http, "TCP"));
    }
    for p in container
        .ports
        .iter()
        .filter(|p| p.expose != Expose::NodePort)
    {
        ports.push(service_port(p.name(), p.port, p.protocol.as_str()));
    }
    if ports.is_empty() {
        return None;
    }
    Some(Service {
        metadata: metadata(meta, meta.name.to_owned()),
        spec: Some(ServiceSpec {
            selector: Some(meta.labels.clone()),
            ports: Some(ports),
            ..ServiceSpec::default()
        }),
        ..Service::default()
    })
}

/// A NodePort Service for the `expose: nodePort` ports only, so the HTTP port never gets a
/// node port allocated. `externalTrafficPolicy: Local` keeps the client IP and makes only the
/// node(s) running the pod answer, which is what "the node is the region" needs.
fn node_service(meta: &TappMeta, app: &TailoredApp) -> Option<Service> {
    let ports: Vec<ServicePort> = app
        .spec
        .deployment
        .container
        .ports
        .iter()
        .filter(|p| p.expose == Expose::NodePort)
        .map(|p| service_port(p.name(), p.port, p.protocol.as_str()))
        .collect();
    if ports.is_empty() {
        return None;
    }
    Some(Service {
        metadata: metadata(meta, node_service_name(meta)),
        spec: Some(ServiceSpec {
            selector: Some(meta.labels.clone()),
            ports: Some(ports),
            type_: Some("NodePort".to_owned()),
            external_traffic_policy: Some("Local".to_owned()),
            ..ServiceSpec::default()
        }),
        ..Service::default()
    })
}

pub fn node_service_name(meta: &TappMeta) -> String {
    format!("{}{NODE_SERVICE_SUFFIX}", meta.name)
}

/// Every Service the app needs: the in-cluster one and, if any port is `nodePort`, the Local
/// NodePort one.
pub fn all(meta: &TappMeta, app: &TailoredApp) -> Vec<Service> {
    [cluster_service(meta, app), node_service(meta, app)]
        .into_iter()
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use kubetailor::crd::{Container, Deployment, Port, Protocol, TailoredAppSpec};

    use super::*;

    fn app(port: Option<i32>, ports: Vec<Port>) -> TailoredApp {
        TailoredApp::new(
            "game",
            TailoredAppSpec {
                labels: BTreeMap::new(),
                deployment: Deployment {
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
                        image: "x".into(),
                        port,
                        ports,
                        replicas: 1,
                        ..Container::default()
                    },
                },
                ingress: None,
                env: None,
                secrets: None,
                git: None,
            },
        )
    }

    fn meta() -> TappMeta {
        TappMeta {
            name: "game".into(),
            namespace: "kubetailor".into(),
            labels: BTreeMap::from([("tapp".to_owned(), "game".to_owned())]),
            oref: OwnerReference::default(),
        }
    }

    fn port(port: i32, protocol: Protocol, expose: Expose) -> Port {
        Port {
            port,
            protocol,
            expose,
            name: None,
        }
    }

    #[test]
    fn http_only_app_keeps_single_named_port() {
        let svc = cluster_service(&meta(), &app(Some(80), vec![])).unwrap();
        let ports = svc.spec.unwrap().ports.unwrap();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].name.as_deref(), Some("game"));
        assert_eq!(ports[0].protocol.as_deref(), Some("TCP"));
        assert!(node_service(&meta(), &app(Some(80), vec![])).is_none());
    }

    #[test]
    fn host_port_udp_lands_in_cluster_service_not_node_service() {
        let a = app(None, vec![port(7777, Protocol::Udp, Expose::Node)]);
        let svc = cluster_service(&meta(), &a).unwrap();
        let ports = svc.spec.unwrap().ports.unwrap();
        assert_eq!(ports[0].name.as_deref(), Some("udp-7777"));
        assert_eq!(ports[0].protocol.as_deref(), Some("UDP"));
        assert!(node_service(&meta(), &a).is_none());
    }

    #[test]
    fn node_port_ports_get_their_own_local_service() {
        let a = app(Some(80), vec![port(27015, Protocol::Udp, Expose::NodePort)]);
        let cluster = cluster_service(&meta(), &a).unwrap().spec.unwrap();
        assert_eq!(cluster.ports.unwrap().len(), 1, "http only");
        let node = node_service(&meta(), &a).unwrap();
        assert_eq!(node.metadata.name.as_deref(), Some("game-node"));
        let spec = node.spec.unwrap();
        assert_eq!(spec.type_.as_deref(), Some("NodePort"));
        assert_eq!(spec.external_traffic_policy.as_deref(), Some("Local"));
        assert_eq!(spec.ports.unwrap()[0].port, 27015);
    }

    #[test]
    fn app_without_any_port_has_no_service() {
        assert!(cluster_service(&meta(), &app(None, vec![])).is_none());
    }
}
