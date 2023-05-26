use std::fmt::Write;

use actix_web::{http::header::ContentType, web, HttpResponse};
use actix_web_flash_messages::IncomingFlashMessages;
use handlebars::Handlebars;
use serde_json::json;

pub async fn login_form(
    hb: web::Data<Handlebars<'_>>,
    flash_messages: IncomingFlashMessages,
) -> HttpResponse {
    let mut error_html = String::new();
    for m in flash_messages.iter() {
        writeln!(error_html, "{}", m.content()).unwrap();
    }
    let data = json!({
        "title": "Login",
        "error_html": error_html,
    });
    let body = hb.render("login", &data).unwrap();

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(body)
}
