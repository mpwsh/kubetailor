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

fn new(meta: &TappMeta, app: &TailoredApp) -> Result<Ingress, Error> {
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

    Ok(Ingress {
        metadata: ObjectMeta {
            name: Some(meta.name.to_owned()),
            namespace: Some(meta.namespace.to_owned()),
            labels: Some(meta.labels.to_owned()),
            annotations: Some(ingress.annotations.clone()),
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

pub async fn deploy(client: &Client, meta: &TappMeta, app: &TailoredApp) -> Result<Ingress, Error> {
    let ingress = new(meta, app)?;
    let api: Api<Ingress> = Api::namespaced(client.to_owned(), &meta.namespace);
    match api.create(&PostParams::default(), &ingress).await {
        Ok(s) => Ok(s),
        Err(kubetailor::kube::Error::Api(e)) if e.code == 409 => update(client, meta, app).await,
        Err(e) => Err(Error::KubeError { source: e }),
    }
}

pub async fn update(client: &Client, meta: &TappMeta, app: &TailoredApp) -> Result<Ingress, Error> {
    let mut ingress = new(meta, app)?;
    let api: Api<Ingress> = Api::namespaced(client.to_owned(), &meta.namespace);
    let existing = api.get(&meta.name).await?;
    ingress.metadata.resource_version = existing.metadata.resource_version;
    // For node-bound apps the DNS target is owned by the placement step, not by the spec:
    // carry the value it wrote over the rebuild instead of flapping back to the spec's.
    if app.spec.is_node_bound() {
        if let Some(target) = existing
            .metadata
            .annotations
            .as_ref()
            .and_then(|a| a.get(crate::placement::DNS_TARGET_ANNOTATION))
        {
            ingress
                .metadata
                .annotations
                .get_or_insert_with(BTreeMap::new)
                .insert(
                    crate::placement::DNS_TARGET_ANNOTATION.to_owned(),
                    target.clone(),
                );
        }
    }

    Ok(api
        .replace(&meta.name, &PostParams::default(), &ingress)
        .await?)
}
