use std::collections::BTreeMap;

use kubetailor::crd::{self, Domains};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Ingress {
    pub base_domain: String,
    pub annotations: BTreeMap<String, String>,
    pub match_labels: BTreeMap<String, String>,
    pub load_balancer_endpoint: String,
    pub class_name: String,
}

impl Ingress {
    pub fn build_annotations(
        &self,
        subdomain: &str,
        base_domain: &str,
    ) -> BTreeMap<String, String> {
        let mut annotations = BTreeMap::new();
        for a in self.annotations.iter() {
            annotations.insert(a.0.to_owned(), a.1.to_owned());
        }
        annotations.insert(
            "external-dns.alpha.kubernetes.io/target".to_owned(),
            self.load_balancer_endpoint.clone(),
        );
        annotations.insert(
            "external-dns.alpha.kubernetes.io/hostname".to_owned(),
            format!("{}.{}", subdomain, base_domain),
        );
        annotations
    }
    pub fn build(&self, domains: Option<Domains>) -> crd::Ingress {
        match domains {
            Some(domains) => {
                let subdomain = domains.shared.clone();
                let custom = domains.custom;
                crd::Ingress {
                    annotations: self.build_annotations(&subdomain, &self.base_domain),
                    class_name: self.class_name.clone(),
                    match_labels: self.match_labels.clone(),
                    domains: Some(Domains {
                        shared: format!("{}.{}", subdomain, self.base_domain),
                        custom,
                    }),
                }
            },
            None => crd::Ingress {
                annotations: BTreeMap::new(),
                class_name: self.class_name.clone(),
                match_labels: self.match_labels.clone(),
                domains: None,
            },
        }
    }
}
