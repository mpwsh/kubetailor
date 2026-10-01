use actix_web::{http::header::LOCATION, HttpRequest, HttpResponse};
use serde_json::json;

use crate::htmx::HtmxRequest;

/// Container every page renders into; htmx navigations target it.
pub const CONTENT_TARGET: &str = "#content";

// Return an opaque 500 while preserving the error root's cause for logging.
pub fn e500<T>(e: T) -> actix_web::Error
where
    T: std::fmt::Debug + std::fmt::Display + 'static,
{
    actix_web::error::ErrorInternalServerError(e)
}

/// Plain browser redirect. For htmx callers use [`redirect`]: XHR follows a 303 silently, so the
/// caller would receive the target's fragment (and swap it wherever its `hx-swap` says) instead
/// of navigating.
pub fn see_other(location: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header((LOCATION, location))
        .finish()
}

/// Redirect that works for both kinds of caller. A browser gets a 303. An htmx request gets a
/// `200` with `HX-Location`, which makes htmx GET the page and swap it into `#content` with the
/// URL pushed, exactly like a sidebar click.
pub fn redirect(req: &HttpRequest, location: &str) -> HttpResponse {
    if req.is_htmx() {
        HttpResponse::Ok()
            .insert_header((
                "HX-Location",
                json!({"path": location, "target": CONTENT_TARGET, "swap": "innerHTML"})
                    .to_string(),
            ))
            .finish()
    } else {
        see_other(location)
    }
}

/// Redirect that must leave the application shell (login, logout): a full page load for htmx
/// callers via `HX-Redirect`, a 303 otherwise.
pub fn redirect_full(req: &HttpRequest, location: &str) -> HttpResponse {
    if req.is_htmx() {
        HttpResponse::Ok()
            .insert_header(("HX-Redirect", location))
            .finish()
    } else {
        see_other(location)
    }
}

/// Shows `message` inside `#form-errors` without touching the rest of the page: the response is
/// retargeted and reswapped from the server (`HX-Retarget` / `HX-Reswap`), so the form keeps its
/// `hx-target` for the success path. htmx only swaps 2xx by default, hence the 200.
pub fn form_error(message: &str) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("HX-Retarget", "#form-errors"))
        .insert_header(("HX-Reswap", "innerHTML"))
        .content_type("text/html; charset=utf-8")
        .body(format!(
            r#"<div role="alert" class="rounded-md border border-red-300 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-700 dark:bg-red-950 dark:text-red-200">{}</div>"#,
            html_escape(message)
        ))
}

pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}
