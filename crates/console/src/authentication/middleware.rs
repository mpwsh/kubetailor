use std::ops::Deref;

use actix_web::{
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    error::InternalError,
    middleware::Next,
    FromRequest, HttpMessage, HttpResponse,
};

use crate::{
    htmx::HtmxRequest,
    session_state::TypedSession,
    utils::{e500, see_other},
};

#[derive(Clone, Debug)]
pub struct UserId(String);

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for UserId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub async fn reject_anonymous_users(
    mut req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let session = {
        let (http_request, payload) = req.parts_mut();
        TypedSession::from_request(http_request, payload).await
    }?;

    match session.get_user().map_err(e500)? {
        Some(user_id) => {
            req.extensions_mut().insert(UserId(user_id));
            next.call(req).await
        }
        None => {
            // An htmx request must not have the login page swapped into `#content`: send it
            // through a full redirect instead. 401 is the honest status; htmx processes
            // HX-Redirect regardless of status code.
            let response = if req.request().is_htmx() {
                HttpResponse::Unauthorized()
                    .insert_header(("HX-Redirect", "/login"))
                    .finish()
            } else {
                see_other("/login")
            };
            let e = anyhow::anyhow!("The user has not logged in");
            Err(InternalError::from_response(e, response).into())
        }
    }
}
