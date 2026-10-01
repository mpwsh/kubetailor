pub use std::collections::BTreeMap;

pub use kube::Resource;
pub use serde::{Deserialize, Serialize, Serializer};

pub use crate::{
    cert_crd::{Certificate, CertificateSpec, Status as CertificateStatus},
    crd::{
        Container, Deployment, Domains, Endpoint, Expose, Ingress, PlacementNode, Port, Protocol,
        TailoredApp, TailoredAppSpec, TailoredAppStatus,
    },
    resources::Resources,
};
