use kube::runtime::controller::Action;
use tokio::time::Duration;

use crate::prelude::*;

/// Actions to be taken when a reconciliation fails - for whatever reason.
/// Prints out the error to `stderr` and requeues the resource for another reconciliation after
/// five seconds.
///
/// # Arguments
/// - `error`: A reference to the `kube::Error` that occurred during reconciliation.
/// - `_context`: Unused argument. Context Data "injected" automatically by kube-rs.
pub fn on_error(app: Arc<TailoredApp>, error: &Error, _context: Arc<ContextData>) -> Action {
    error!("Reconciliation error:\n{error:?}.\n{app:?}");
    Action::requeue(Duration::from_secs(5))
}

/// All errors possible to occur during reconciliation
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Any error originating from the `kube-rs` crate
    #[error("Kubernetes reported error: {source}")]
    KubeError {
        #[from]
        source: kube::Error,
    },
    /// Error in user input or TailoredApp resource definition, typically missing fields.
    #[error("Invalid TailoredApp CRD: {0}")]
    UserInputError(String),
    #[error("MissingObjectKey: {0}")]
    MissingObjectKey(&'static str),
    #[error("UpdateDeployment: {0}")]
    UpdateDeployment(#[source] kube::Error),
    #[error("DeleteDeployment: {0}")]
    DeleteDeployment(#[source] kube::Error),
}
