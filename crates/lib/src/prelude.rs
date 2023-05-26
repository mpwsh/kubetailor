pub use std::collections::BTreeMap;

pub use kube::Resource;
pub use serde::{Deserialize, Serialize};

pub use crate::crd::{Container, Deployment, Domains, Ingress, TailoredApp, TailoredAppSpec};
