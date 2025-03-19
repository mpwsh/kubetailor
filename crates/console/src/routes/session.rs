use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use anyhow::Result;
use log::error;
use portier::Client;
use serde::{Deserialize, Serialize};

use crate::{
    errors::ApiError,
    session_state::TypedSession,
    utils::{e500, see_other},
};

#[derive(Deserialize)]
pub struct AuthForm {
    pub email: String,
}

#[derive(Deserialize)]
pub struct VerifyForm {
    pub id_token: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct UserData {
    pub email: Option<String>,
}

pub async fn claim(
    form: Option<web::Form<VerifyForm>>,
    client: web::Data<Client>,
    session: TypedSession,
) -> HttpResponse {
    match form {
        Some(form) => {
            let verification = client.verify(&form.id_token).await.map_err(|err| {
                error!("Portier verify error: {}", err);
                ApiError::PortierVerifyError(err.to_string())
            });
            let mut email = String::new();
            match verification {
                Ok(addr) => email = addr,
                Err(e) => {
                    FlashMessage::info(format!(
                        "{e:?}. Unable to initialize session. Please try again"
                    ))
                    .send();
                    see_other("/login");
                },
            };
            session.insert_user(&email).unwrap();
            session.renew();
            see_other("/")
        },
        None => {
            FlashMessage::info(
                "Internal server error. Unable to initialize session. Please try again",
            )
            .send();
            see_other("/login")
        },
    }
}

pub async fn whoami(session: TypedSession) -> Result<HttpResponse, ApiError> {
    let email = session.get_user().unwrap();
    Ok(HttpResponse::Ok().json(UserData { email }))
}

pub async fn authenticate(form: web::Form<AuthForm>, client: web::Data<Client>) -> HttpResponse {
    match client.start_auth(&form.email).await {
        Ok(url) => see_other(url.as_ref()),
        Err(err) => {
            FlashMessage::info(format!(
                "Internal server error. Unable to initialize session. Please try again\n{err}"
            ))
            .send();
            see_other("/login")
        },
    }
}

pub async fn logout(session: TypedSession) -> Result<HttpResponse, actix_web::Error> {
    if session.get_user().map_err(e500)?.is_none() {
        Ok(see_other("/login"))
    } else {
        session.logout();
        FlashMessage::info("You have successfully logged out.").send();
        Ok(see_other("/login"))
    }
}
