use crate::prelude::*;
/// Context injected with each `reconcile` and `on_error` method invocation.
pub struct ContextData {
    /// Kubernetes client to make Kubernetes API requests with. Required for K8S resource management.
    pub client: Client,
    /// Whether the cluster has the flint controller's CRDs: nodes can be asked for and ports
    /// opened. Checked once at start-up.
    pub flint: bool,
}

impl ContextData {
    /// Constructs a new instance of ContextData.
    ///
    /// # Arguments:
    ///     - `client`: A Kubernetes client to make Kubernetes REST API requests with. Resources
    /// will be created and deleted with this client.
    ///     - `flint`: whether `flint.mpw.sh` objects exist in this cluster.
    ///
    pub fn new(client: Client, flint: bool) -> Self {
        ContextData { client, flint }
    }
}
