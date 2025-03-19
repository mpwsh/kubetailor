use std::fmt::Write;

use actix_web::{http::header::ContentType, web, HttpResponse};
use actix_web_flash_messages::IncomingFlashMessages;
use handlebars::Handlebars;
use serde_json::json;

pub async fn page(
    hb: web::Data<Handlebars<'_>>,
    flash_messages: IncomingFlashMessages,
) -> HttpResponse {
    let mut error = String::new();
    let mut has_messages = false;

    for m in flash_messages.iter() {
        has_messages = true;
        writeln!(error, "{}", m.content()).unwrap();
    }

    if !has_messages {
        error = "Internal server error".to_string();
    }

    log::error!("{error}");

    let data = json!({
        "title": "Error",
        "status_code": "500",
        "error": error,
        "initial": true
    });

    let body = hb.render("error", &data).unwrap();
    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(body)
}
