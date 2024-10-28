pub mod delete;
pub mod edit;
pub mod health;
pub mod logs;
pub mod new;

use crate::routes::prelude::*;

pub async fn get(tapp_name: &str, owner: &str, kubetailor: web::Data<Kubetailor>) -> TappConfig {
    let items: Vec<String> = kubetailor
        .client
        .get(format!(
            "{}/list?owner={}&filter=name",
            kubetailor.url, owner
        ))
        .send()
        .await
        .unwrap()
        .json::<Vec<String>>()
        .await
        .unwrap();

    if items.into_iter().any(|name| name == tapp_name) {
        kubetailor
            .client
            .get(format!("{}/{}?owner={}", kubetailor.url, tapp_name, owner))
            .send()
            .await
            .unwrap()
            .json::<TappConfig>()
            .await
            .unwrap()
    } else {
        TappConfig::default()
    }
}

pub async fn deploying(
    hb: web::Data<Handlebars<'_>>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();

    let data = json!({
        "user": user,
    });

    let body = hb.render("new", &data).unwrap();

    Ok(HttpResponse::Ok()
        .insert_header(ContentType(mime::TEXT_HTML))
        .body(body))
}
