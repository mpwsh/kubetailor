pub use actix_web::{http::header::ContentType, web, HttpMessage, HttpRequest, HttpResponse};
pub use actix_web_flash_messages::FlashMessage;
pub use handlebars::Handlebars;
pub use reqwest::StatusCode;
pub use serde::{Deserialize, Serialize};
pub use serde_json::json;

pub use crate::{
    authentication::UserId,
    config::Kubetailor,
    models::*,
    routes::dashboard::tapp,
    utils::{e500, see_other},
};
