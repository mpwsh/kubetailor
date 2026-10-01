use crate::routes::prelude::*;

/// Element ids of the two self-rendering parts of the log viewer.
const VIEW_ID: &str = "logs-view";
const PANE_ID: &str = "log-pane";

const INTERVALS: &[(u32, &str)] = &[(5, "5s"), (10, "10s"), (30, "30s"), (60, "1m"), (300, "5m")];

#[derive(Debug, Deserialize)]
pub struct Form {
    name: Option<String>,
    query: Option<String>,
    interval: Option<u32>,
}

pub async fn page(
    hb: web::Data<Handlebars<'_>>,
    params: web::Query<Form>,
    kubetailor: web::Data<Kubetailor>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();

    let name = params.name.clone().filter(|n| !n.is_empty());
    let query = params.query.clone().filter(|q| !q.trim().is_empty());
    let interval = params
        .interval
        .filter(|i| INTERVALS.iter().any(|(s, _)| s == i))
        .unwrap_or(10);

    let deployments = deployments::names(&user, &kubetailor).await?;

    let logs: Vec<String> = match &name {
        Some(name) if deployments.contains(name) => {
            let mut url = format!("{}/{}/logs?owner={}", kubetailor.url, name, user);
            if let Some(query) = &query {
                url.push_str(&format!("&query={}", urlencoding(query)));
            }
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
                })?
        }
        _ => Vec::new(),
    };

    let intervals: Vec<Value> = INTERVALS
        .iter()
        .map(|(seconds, label)| {
            json!({"seconds": seconds, "label": label, "selected": *seconds == interval})
        })
        .collect();
    let data = json!({
        "title": "Application logs",
        "initial": !req.is_htmx(),
        "deployments": deployments,
        "name": name,
        "query": query,
        "interval": interval,
        "intervals": intervals,
        "logs": logs,
        "user": user,
    });
    let template = if req.targets(PANE_ID) {
        "logs/pane"
    } else if req.targets(VIEW_ID) {
        "logs/view"
    } else {
        "logs"
    };
    let body = hb
        .render(template, &data)
        .map_err(|e| ApiError::InternalError(format!("Template error: {}", e)))?;
    Ok(HttpResponse::Ok().body(body))
}

/// Percent-encodes a query string value.
fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
