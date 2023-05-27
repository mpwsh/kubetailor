pub use std::collections::BTreeMap;

pub use kube::Resource;
pub use serde::{Deserialize, Serialize, Serializer};

pub use crate::crd::{Container, Deployment, Domains, Ingress, TailoredApp, TailoredAppSpec};
