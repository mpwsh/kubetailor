use crate::{
    k8s_openapi::api::{apps::v1::Deployment, core::v1::Pod, networking::v1::Ingress},
    prelude::*,
};

pub struct Resources {
    pub cert: Option<Vec<Certificate>>,
    pub pods: Option<Vec<Pod>>,
    pub deployment: Option<Vec<Deployment>>,
    pub ingress: Option<Vec<Ingress>>,
}
