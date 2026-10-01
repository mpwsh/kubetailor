use crate::routes::prelude::*;

/// The backend's health document for one deployment (`deployment.ready`, `deployment.replicas`,
/// `deployment.state`, `domains.domains`, `domains.dns`, `domains.ssl`). Rendered server-side
/// into the deployments table and the deploy progress view.
pub async fn get(name: String, user: String, kubetailor: &Kubetailor) -> Result<Value, ApiError> {
    let url = format!(
        "{url}/{name}/health?owner={user}&filter=name",
        url = kubetailor.url
    );

    kubetailor
        .client
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            log::error!("Kubetailor client error: {} for URL: {}", e, url);
            ApiError::InternalError(format!("Failed to fetch status: {}", e))
        })?
        .error_for_status()
        .map_err(|e| ApiError::InternalError(format!("Status endpoint failed: {}", e)))?
        .json::<Value>()
        .await
        .map_err(|e| {
            log::error!("Failed to parse kubetailor response: {}", e);
            ApiError::InternalError(format!("Failed to deployment status response: {}", e))
        })
}
