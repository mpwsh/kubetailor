use k8s_openapi::api::networking::v1::{
    IPBlock, NetworkPolicyEgressRule, NetworkPolicyIngressRule, NetworkPolicyPeer,
    NetworkPolicyPort, NetworkPolicySpec,
};

use crate::prelude::*;

fn new(meta: &TappMeta, app: &TailoredApp) -> NetworkPolicy {
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
            ingress: Some(vec![NetworkPolicyIngressRule {
                from: Some(vec![
                    NetworkPolicyPeer {
                        pod_selector: Some(LabelSelector {
                            match_labels: Some(app.spec.ingress.match_labels.to_owned()),
                            ..LabelSelector::default()
                        }),
                        ..NetworkPolicyPeer::default()
                    },
                    NetworkPolicyPeer {
                        pod_selector: Some(LabelSelector {
                            match_labels: Some(meta.labels.to_owned()),
                            ..LabelSelector::default()
                        }),
                        ..NetworkPolicyPeer::default()
                    },
                ]),
                ..NetworkPolicyIngressRule::default()
            }]),
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
                            match_labels: Some(meta.labels.to_owned()),
                            ..LabelSelector::default()
                        }),
                        ..NetworkPolicyPeer::default()
                    }]),
                    ..NetworkPolicyEgressRule::default()
                },
                // Allow egress to DNS (kube-dns or CoreDNS)
                NetworkPolicyEgressRule {
                    to: Some(vec![NetworkPolicyPeer {
                        namespace_selector: Some(LabelSelector::default()), /* modify this to select kube-system namespace */
                        pod_selector: Some(LabelSelector {
                            match_labels: Some(BTreeMap::from_iter(vec![
                                ("k8s-app".to_string(), "kube-dns".to_string()), /* or use "k8s-app".to_string(), "coredns".to_string() depending on your DNS service */
                            ])),
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
    }
}

pub async fn deploy(
    client: &Client,
    meta: &TappMeta,
    app: &TailoredApp,
) -> Result<NetworkPolicy, Error> {
    let netpol = new(meta, app);
    let api: Api<NetworkPolicy> = Api::namespaced(client.clone(), &meta.namespace);
    match api.create(&PostParams::default(), &netpol).await {
        Ok(cm) => Ok(cm),
        Err(kube::Error::Api(e)) if e.code == 409 => update(client, meta, app).await,
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

pub async fn update(
    client: &Client,
    meta: &TappMeta,
    app: &TailoredApp,
) -> Result<NetworkPolicy, Error> {
    let mut netpol = new(meta, app);
    let api: Api<NetworkPolicy> = Api::namespaced(client.to_owned(), &meta.namespace);

    let resource_version = api.get(&meta.name).await?.metadata.resource_version;
    netpol.metadata.resource_version = resource_version;

    Ok(api
        .replace(&meta.name, &PostParams::default(), &netpol)
        .await?)
}
