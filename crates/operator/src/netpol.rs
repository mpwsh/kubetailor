use kubetailor::k8s_openapi::api::networking::v1::{
    IPBlock, NetworkPolicyEgressRule, NetworkPolicyIngressRule, NetworkPolicyPeer,
    NetworkPolicyPort, NetworkPolicySpec,
};

use crate::prelude::*;

/// Who may talk to the pods:
/// - the ingress controller (pods matching `ingress.matchLabels`, any namespace), when the app
///   has an ingress section;
/// - pods of the same owner/group (same labels minus `tapp`);
/// - the internet, but only on ports exposed at the node (`expose: node` / `nodePort`), each on
///   its own protocol and port. Everything else stays closed.
fn ingress_rules(
    app: &TailoredApp,
    peer_labels: &BTreeMap<String, String>,
) -> Vec<NetworkPolicyIngressRule> {
    let mut from = Vec::new();
    if let Some(ingress) = &app.spec.ingress {
        from.push(NetworkPolicyPeer {
            namespace_selector: Some(LabelSelector::default()),
            pod_selector: Some(LabelSelector {
                match_labels: Some(ingress.match_labels.to_owned()),
                ..LabelSelector::default()
            }),
            ..NetworkPolicyPeer::default()
        });
    }
    from.push(NetworkPolicyPeer {
        pod_selector: Some(LabelSelector {
            match_labels: Some(peer_labels.to_owned()),
            ..LabelSelector::default()
        }),
        ..NetworkPolicyPeer::default()
    });
    let mut rules = vec![NetworkPolicyIngressRule {
        from: Some(from),
        ..NetworkPolicyIngressRule::default()
    }];
    for p in app.spec.external_ports() {
        rules.push(NetworkPolicyIngressRule {
            from: Some(vec![NetworkPolicyPeer {
                ip_block: Some(IPBlock {
                    cidr: "0.0.0.0/0".to_string(),
                    except: None,
                }),
                ..NetworkPolicyPeer::default()
            }]),
            ports: Some(vec![NetworkPolicyPort {
                protocol: Some(p.protocol.as_str().to_string()),
                port: Some(IntOrString::Int(p.port)),
                end_port: None,
            }]),
        });
    }
    rules
}

pub fn new(meta: &TappMeta, app: &TailoredApp) -> NetworkPolicy {
    let mut labels = meta.labels.clone();
    labels.remove("tapp");
    NetworkPolicy {
        metadata: ObjectMeta {
            name: Some(meta.name.to_owned()),
            namespace: Some(meta.namespace.to_owned()),
            owner_references: Some(vec![meta.oref.to_owned()]),
            labels: Some(meta.labels.to_owned()),
            ..ObjectMeta::default()
        },
        spec: Some(NetworkPolicySpec {
            pod_selector: LabelSelector {
                match_labels: Some(meta.labels.to_owned()),
                ..LabelSelector::default()
            },
            ingress: Some(ingress_rules(app, &labels)),
            egress: Some(vec![
                //Allow egress to the internet, block internal networks
                NetworkPolicyEgressRule {
                    to: Some(vec![NetworkPolicyPeer {
                        ip_block: Some(IPBlock {
                            cidr: "0.0.0.0/0".to_string(),
                            except: Some(vec![
                                "10.0.0.0/8".to_string(),
                                "192.168.0.0/16".to_string(),
                                "172.16.0.0/20".to_string(),
                            ]),
                        }),
                        ..NetworkPolicyPeer::default()
                    }]),
                    ..NetworkPolicyEgressRule::default()
                },
                // Allow egress to all pods with the same labels
                NetworkPolicyEgressRule {
                    to: Some(vec![NetworkPolicyPeer {
                        pod_selector: Some(LabelSelector {
                            match_labels: Some(labels.to_owned()),
                            ..LabelSelector::default()
                        }),
                        ..NetworkPolicyPeer::default()
                    }]),
                    ..NetworkPolicyEgressRule::default()
                },
                // Allow egress to DNS (kube-dns or CoreDNS)
                NetworkPolicyEgressRule {
                    to: Some(vec![NetworkPolicyPeer {
                        namespace_selector: Some(LabelSelector::default()),
                        pod_selector: Some(LabelSelector {
                            match_labels: Some(BTreeMap::from_iter(vec![(
                                "k8s-app".to_string(),
                                "kube-dns".to_string(),
                            )])),
                            ..LabelSelector::default()
                        }),
                        ..NetworkPolicyPeer::default()
                    }]),
                    ports: Some(vec![
                        NetworkPolicyPort {
                            protocol: Some("UDP".to_string()),
                            port: Some(IntOrString::Int(53)),
                            end_port: None,
                        },
                        NetworkPolicyPort {
                            protocol: Some("TCP".to_string()),
                            port: Some(IntOrString::Int(53)),
                            end_port: None,
                        },
                    ]),
                },
            ]),
            policy_types: Some(vec!["Ingress".to_string(), "Egress".to_string()]),
        }),
        ..NetworkPolicy::default()
    }
}
