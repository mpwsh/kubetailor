use crate::routes::prelude::*;

#[derive(Deserialize)]
pub struct Form {
    pub name: String,
    pub query: Option<String>,
}

pub async fn get(
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

    let url = if let Some(query) = &params.query {
        format!(
            "{}/{}/logs?owner={}&query={}",
            kubetailor.url, params.name, user, query
        )
    } else {
        format!("{}/{}/logs?owner={}", kubetailor.url, params.name, user)
    };

    // Send the request
    let logs = kubetailor
        .client
        .get(url)
        .send()
        .await
        .expect("Failed to send request")
        .json::<Vec<String>>()
        .await
        .unwrap();
    let data = json!({
        "title": "Application logs",
        "return_url": "/dashboard",
        "big_container": true,
        "logs": logs,
        "user": user,
    });

    let body = hb.render("logs", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
}
