use actix_web::{
    body::BoxBody,
    dev::ServiceResponse,
    http::{header::ContentType, StatusCode},
    middleware::{ErrorHandlerResponse, ErrorHandlers},
    web, HttpRequest, HttpResponse, ResponseError, Result,
};
use actix_web_flash_messages::IncomingFlashMessages;
use handlebars::Handlebars;
use reqwest::Error as ReqwestError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt::{self, Write};

#[derive(Debug, Serialize, Deserialize)]
pub enum ApiError {
    PortierVerifyError(String),
    ConnectionError(String),
    DatabaseError(String),
    RequestError(String),
    InternalError(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ApiError::PortierVerifyError(msg) => write!(f, "Authentication error: {}", msg),
            ApiError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            ApiError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            ApiError::RequestError(msg) => write!(f, "Request error: {}", msg),
            ApiError::InternalError(msg) => write!(f, "Internal server error: {}", msg),
        }
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        let status = match self {
            ApiError::PortierVerifyError(_) => StatusCode::UNAUTHORIZED,
            ApiError::ConnectionError(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::RequestError(_) => StatusCode::BAD_REQUEST,
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let error_msg = self.to_string();

        // Create a basic error response
        HttpResponse::build(status)
            .content_type(ContentType::html())
            .body(error_msg)
    }
}

impl From<ReqwestError> for ApiError {
    fn from(err: ReqwestError) -> Self {
        if err.is_connect() {
            ApiError::ConnectionError(err.to_string())
        } else if err.is_timeout() {
            ApiError::ConnectionError("Request timed out".to_string())
        } else {
            ApiError::RequestError(err.to_string())
        }
    }
}

pub fn error_handlers() -> ErrorHandlers<BoxBody> {
    ErrorHandlers::new()
        .handler(StatusCode::NOT_FOUND, not_found)
        .handler(StatusCode::INTERNAL_SERVER_ERROR, internal_error)
        .handler(StatusCode::BAD_REQUEST, bad_request)
        .handler(StatusCode::SERVICE_UNAVAILABLE, service_unavailable)
}

fn not_found<B>(res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<BoxBody>> {
    let response = get_error_response(&res, "Page not found");
    Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
        res.into_parts().0,
        response.map_into_left_body(),
    )))
}

fn internal_error<B>(res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<BoxBody>> {
    let response = get_error_response(&res, "Internal server error");
    Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
        res.into_parts().0,
        response.map_into_left_body(),
    )))
}

fn bad_request<B>(res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<BoxBody>> {
    let response = get_error_response(&res, "Bad request");
    Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
        res.into_parts().0,
        response.map_into_left_body(),
    )))
}

fn service_unavailable<B>(res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<BoxBody>> {
    let response = get_error_response(&res, "Service unavailable");
    Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
        res.into_parts().0,
        response.map_into_left_body(),
    )))
}

pub fn page(
    hb: &Handlebars,
    status: StatusCode,
    error: &str,
    flash_messages: Option<&IncomingFlashMessages>,
) -> HttpResponse {
    let mut error_html = String::new();

    // Only iterate if flash_messages is Some
    if let Some(messages) = flash_messages {
        for m in messages.iter() {
            writeln!(error_html, "{}", m.content()).unwrap();
        }
    }

    let error = format!("{error} - {error_html}");
    let data = json!({
        "initial": true,
        "error": error,
        "status_code": status.as_str(),
        "url": "/home",
        "title": format!("Error {}", status.as_str())
    });

    match hb.render("error", &data) {
        Ok(body) => HttpResponse::build(status)
            .content_type(ContentType::html())
            .body(body),
        Err(_) => HttpResponse::build(status)
            .content_type(ContentType::plaintext())
            .body(error.to_string()),
    }
}

// For middleware errors

pub fn get_error_response<B>(res: &ServiceResponse<B>, error: &str) -> HttpResponse {
    let request = res.request();

    // Get the detailed error from the response error
    let detailed_error = if let Some(err) = res.response().error() {
        if let Some(api_err) = err.as_error::<ApiError>() {
            api_err.to_string()
        } else {
            error.to_string()
        }
    } else {
        error.to_string()
    };

    log::error!(
        "Error occurred: Status: {}, Message: {}, Details: {}, Path: {}",
        res.status(),
        error,
        detailed_error,
        res.request().path()
    );

    // Retrieve Handlebars and flash messages from the app data
    let hb = request
        .app_data::<web::Data<Handlebars>>()
        .map(|t| t.get_ref());

    let flash_messages = request
        .app_data::<web::Data<IncomingFlashMessages>>()
        .map(|fm| fm.get_ref());

    match hb {
        Some(hb) => page(hb, res.status(), &detailed_error, flash_messages),
        None => HttpResponse::build(res.status())
            .content_type(ContentType::plaintext())
            .body(detailed_error),
    }
}

pub async fn handle_not_found(
    req: HttpRequest,
    hb: web::Data<Handlebars<'_>>,
) -> Result<HttpResponse, actix_web::Error> {
    let status = StatusCode::NOT_FOUND;
    let error_msg = "Page not found";

    log::error!("404 Error: Path: {}", req.path());

    // Retrieve flash messages from the app data, passing None if unavailable
    let flash_messages = req
        .app_data::<web::Data<IncomingFlashMessages>>()
        .map(|fm| fm.get_ref());

    // Call page with flash_messages as an Option
    Ok(page(hb.get_ref(), status, error_msg, flash_messages))
}
