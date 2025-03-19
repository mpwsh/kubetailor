use crate::routes::prelude::*;
#[derive(Debug, Deserialize)]
pub struct Form {
    pub name: String,
}

pub async fn get(
    name: String,
    user: String,
    kubetailor: web::Data<Kubetailor>,
) -> Result<Value, ApiError> {
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
        .json::<Value>()
        .await
        .map_err(|e| {
            log::error!("Failed to parse kubetailor response: {}", e);
            ApiError::InternalError(format!("Failed to deployment status response: {}", e))
        })
}

pub async fn handler(
    params: Option<web::Query<Form>>,
    name: Option<String>,
    kubetailor: web::Data<Kubetailor>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();
    let name = if let Some(params) = params {
        params.name.clone()
    } else {
        name.unwrap()
    };

    let status = get(name, user, kubetailor).await?;
    Ok(HttpResponse::Ok().json(status))
}
