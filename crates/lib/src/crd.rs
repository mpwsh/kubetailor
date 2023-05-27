use kube::CustomResource;
use schemars::JsonSchema;

use crate::prelude::*;

/// Struct corresponding to the Specification (`spec`) part of the `TailoredApp` resource, directly
/// reflects context of the `deploy/crd.yaml` file to be found in this repository.
/// The `TailoredApp` struct will be generated by the `CustomResource` derive macro.
#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
    group = "mpw.sh",
    version = "v1",
    kind = "TailoredApp",
    plural = "tailoredapps",
    derive = "PartialEq",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct TailoredAppSpec {
    pub labels: BTreeMap<String, String>,
    pub deployment: Deployment,
    pub ingress: Ingress,
    pub env_vars: BTreeMap<String, String>,
    pub secrets: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct Container {
    pub image: String,
    pub port: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_mounts: Option<BTreeMap<String, String>>,
    pub replicas: i32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Ingress {
    pub annotations: BTreeMap<String, String>,
    pub class_name: String,
    pub domains: Domains,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
pub struct Deployment {
    pub annotations: BTreeMap<String, String>,
    pub container: Container,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
pub struct Domains {
    pub shared: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<String>,
}
