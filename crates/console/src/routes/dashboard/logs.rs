use crate::routes::prelude::*;

#[derive(Debug, Deserialize)]
pub struct Form {
    name: Option<String>,
    query: Option<String>,
    plain: Option<bool>,
}

pub async fn page(
    hb: web::Data<Handlebars<'_>>,
    params: Option<web::Query<Form>>,
    kubetailor: web::Data<Kubetailor>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();
    // Only fetch logs if we have params and a name parameter
    let logs: Option<Vec<String>> = if let Some(params) = &params {
        if let Some(name) = &params.name {
            let url = if let Some(query) = &params.query {
                format!(
                    "{}/{}/logs?owner={}&query={}",
                    kubetailor.url, name, user, query
                )
            } else {
                format!("{}/{}/logs?owner={}", kubetailor.url, name, user)
            };
            // Send the request with proper error handling
            Some(
                kubetailor
                    .client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| {
                        log::error!("Kubetailor client error: {} for URL: {}", e, url);
                        ApiError::InternalError(format!("Failed to fetch logs: {}", e))
                    })?
                    .json::<Vec<String>>()
                    .await
                    .map_err(|e| {
                        log::error!("Failed to parse kubetailor response: {}", e);
                        ApiError::InternalError(format!("Failed to parse logs response: {}", e))
                    })?,
            )
        } else {
            None
        }
    } else {
        None
    };
    // Return JSON if plain=true is set
    if params.as_ref().and_then(|p| p.plain).unwrap_or(false) {
        return Ok(HttpResponse::Ok().json(logs.unwrap_or_default()));
    }
    let name = params.as_ref().and_then(|p| p.name.clone());
    let data = json!({
        "title": "Application logs",
        "initial": !req.is_htmx(),
        "logs": logs,
        "user": user,
        "name": name,
    });
    let body = hb
        .render("logs", &data)
        .map_err(|e| ApiError::InternalError(format!("Template error: {}", e)))?;
    Ok(HttpResponse::Ok().body(body))
}
