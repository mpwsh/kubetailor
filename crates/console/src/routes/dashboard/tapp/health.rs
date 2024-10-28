use crate::routes::prelude::*;

pub async fn get(
    tapp_name: &str,
    owner: &str,
    kubetailor: web::Data<Kubetailor>,
) -> serde_json::Value {
    kubetailor
        .client
        .get(format!(
            "{url}/{tapp_name}/health?owner={owner}&filter=name",
            url = kubetailor.url
        ))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()
}
