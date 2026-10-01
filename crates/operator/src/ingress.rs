use kubetailor::k8s_openapi::api::networking::v1::{
    HTTPIngressPath, HTTPIngressRuleValue, IngressBackend, IngressRule, IngressServiceBackend,
    IngressSpec, IngressTLS, ServiceBackendPort,
};

use crate::prelude::*;

/// The ingress config and HTTP port an app needs before an Ingress can be built; a user error
/// otherwise, so the reconciler reports it instead of producing a broken object.
fn requirements(app: &TailoredApp) -> Result<(&kubetailor::crd::Ingress, i32), Error> {
    let ingress = app.spec.ingress.as_ref().ok_or_else(|| {
        Error::UserInputError("ingress domains given without an ingress section".to_owned())
    })?;
    let port = app.spec.deployment.container.port.ok_or_else(|| {
        Error::UserInputError(
            "ingress domains need `container.port` (the HTTP port) to route to".to_owned(),
        )
    })?;
    Ok((ingress, port))
}

pub fn new(meta: &TappMeta, app: &TailoredApp) -> Result<Ingress, Error> {
    let (ingress, port) = requirements(app)?;
    let app = app.spec.clone();
    let paths = vec![HTTPIngressPath {
        path: Some(String::from("/")),
        path_type: String::from("Prefix"),
        backend: IngressBackend {
            resource: None,
            service: Some(IngressServiceBackend {
                name: meta.name.to_owned(),
                port: Some(ServiceBackendPort {
                    name: None,
                    number: Some(port),
                }),
            }),
        },
    }];
    let add_domains = app.hostnames();

    //create ingress rules
    let rules: Vec<IngressRule> = add_domains
        .iter()
        .map(|host| IngressRule {
            host: Some(host.to_owned()),
            http: Some(HTTPIngressRuleValue {
                paths: paths.clone(),
            }),
        })
        .collect();

    let owner = app.labels.get("owner").ok_or_else(|| {
        Error::UserInputError("the `owner` label is required (it names the TLS secret)".to_owned())
    })?;

    // For node-bound apps the external-dns target is owned by the placement step (it is the
    // node's IP, known only once the pod is scheduled), so the spec's value is not applied:
    // under server-side apply, a key we never send is a key we never fight over.
    let mut annotations = ingress.annotations.clone();
    if app.is_node_bound() {
        annotations.remove(crate::placement::DNS_TARGET_ANNOTATION);
    }

    Ok(Ingress {
        metadata: ObjectMeta {
            name: Some(meta.name.to_owned()),
            namespace: Some(meta.namespace.to_owned()),
            labels: Some(meta.labels.to_owned()),
            annotations: Some(annotations),
            owner_references: Some(vec![meta.oref.to_owned()]),
            ..ObjectMeta::default()
        },
        status: None,
        spec: Some(IngressSpec {
            default_backend: None,
            ingress_class_name: Some(ingress.class_name.clone()),
            rules: { Some(rules) },
            tls: Some(vec![IngressTLS {
                hosts: Some(add_domains),
                secret_name: Some(format!("{}-{owner}-kubetailor-tls", meta.name)),
            }]),
        }),
    })
}
