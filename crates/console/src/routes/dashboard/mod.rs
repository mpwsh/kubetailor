use std::collections::HashMap;

use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use handlebars::Handlebars;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    config::Kubetailor,
    errors::ApiError,
    session_state::TypedSession,
    utils::{e500, see_other},
};

#[derive(Deserialize)]
pub struct BasicForm {
    pub name: String,
    pub loading: Option<bool>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct TappConfig {
    pub name: String,
    pub group: Option<String>,
    #[serde(skip_deserializing)]
    pub owner: String,
    pub domains: Domains,
    pub container: Container,
    pub env_vars: Option<HashMap<String, String>>,
    pub secrets: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct Domains {
    pub custom: Option<String>,
    pub shared: String,
}

#[derive(Deserialize, Serialize, Default)]
pub struct Container {
    pub image: String,
    pub replicas: u32,
    pub port: u32,
    pub volumes: Option<HashMap<String, String>>,
    pub files: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Serialize, Debug)]
struct Metadata {
    name: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct Tapp {
    metadata: Metadata,
}
#[derive(Deserialize, Serialize, Debug)]
struct TappListResponse {
    metadata: Metadata,
}
pub async fn dashboard(
    hb: web::Data<Handlebars<'_>>,
    kubetailor: web::Data<Kubetailor>,
    session: TypedSession,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };

    let items: Vec<String> = kubetailor
        .client
        .get(format!(
            "{}/list?owner={}&filter=name",
            kubetailor.url, user
        ))
        .send()
        .await
        .unwrap()
        .json::<Vec<String>>()
        .await
        .unwrap();

    let data = json!({
        "title": "Dashboard",
        "head": "Tapps",
        "deployments": items,
        "home": true,
        "action": "New",
        "user": user,
    });
    let body = hb.render("home", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
}

pub async fn new_form(
    hb: web::Data<Handlebars<'_>>,
    session: TypedSession,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    let data = json!({
        "title": "Editing deployment",
        "head": "New Tapp",
        "is_form": true,
        "show_back": true,
        "action": "Deploy",
        "user": user,
    });
    let body = hb.render("forms/new", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
}
pub async fn new(
    mut tapp: web::Json<TappConfig>,
    session: TypedSession,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    tapp.owner = user;

    let res = kubetailor
        .client
        .post(&kubetailor.url)
        .json(&tapp)
        .send()
        .await;

    match res {
        Ok(response) => {
            if response.status().is_success() {
                Ok(see_other(&format!("/dashboard/loading?name={}", tapp.name)))
            } else {
                // Ok(HttpResponse::BadRequest().body(response.text().await.unwrap().to_string()))
                FlashMessage::info(format!(
                    "Internal server error. Unable to initialize session. Please try again\n{err}",
                    err = response.text().await.unwrap(),
                ))
                .send();
                Ok(see_other("/dashboard/new"))
            }
        },
        Err(e) => Ok(HttpResponse::BadRequest().body(e.to_string())),
    }
}

async fn get(tapp_name: &str, owner: &str, kubetailor: web::Data<Kubetailor>) -> TappConfig {
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

pub async fn view(
    hb: web::Data<Handlebars<'_>>,
    params: web::Query<BasicForm>,
    session: TypedSession,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    let tapp = get(&params.name, &user, kubetailor.clone()).await;
    let data = json!({
        "title": format!("{} Details", params.name),
        "head": format!("{} Details", params.name),
        "show_back": true,
        "view": true,
        "action": "Edit",
        "user": user,
        "tapp": tapp,
    });
    let body = hb.render("view", &data).unwrap();
    if tapp.name.is_empty() {
        Ok(HttpResponse::NotFound().body(""))
    } else {
        Ok(HttpResponse::Ok().body(body))
    }
}
pub async fn edit_form(
    hb: web::Data<Handlebars<'_>>,
    session: TypedSession,
    params: web::Query<BasicForm>,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    let mut tapp = get(&params.name, &user, kubetailor.clone()).await;
    //todo replace when merging server code here
    tapp.domains.shared = tapp.domains.shared.replace(".mpw.sh", "");
    let data = json!({
        "title": "Editing deployment",
        "head": format!("Editing {}", params.name),
        "is_form": true,
        "show_back": true,
        "custom_enabled": tapp.domains.custom.is_some(),
        "action": "Save",
        "tapp": tapp,
        "user": user,
    });

    let body = hb.render("forms/edit", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
}
pub async fn edit(
    mut tapp: web::Json<TappConfig>,
    session: TypedSession,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    let old_tapp = get(&tapp.name, &user, kubetailor.clone()).await;
    if old_tapp.name.is_empty() {
        return Ok(HttpResponse::NotFound().body(format!("Tapp: {} -not found-", tapp.name)));
    }
    tapp.name = old_tapp.name.clone();
    tapp.owner = user;
    kubetailor
        .client
        .put(&kubetailor.url)
        .json(&tapp)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    Ok(see_other(&format!("/dashboard/loading?name={}", tapp.name)))
}

pub async fn check_shared_domain(
    session: TypedSession,
    params: web::Query<BasicForm>,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    let tapp = get(&params.name, &user, kubetailor.clone()).await;
    if tapp.name.is_empty() {
        return Ok(HttpResponse::BadRequest().body(format!("Tapp: {} -not found-", tapp.name)));
    };

    let url = url::Url::parse(&format!("https://{}", tapp.domains.shared))
        .unwrap()
        .to_string();
    log::error!("Using URL: {}", tapp.domains.shared);
    match kubetailor.client.get(url).send().await {
        Ok(response) => match response.status() {
            StatusCode::OK => Ok(HttpResponse::Ok().finish()),
            StatusCode::NOT_FOUND => Ok(HttpResponse::NotFound().finish()),
            _ => Ok(HttpResponse::Created().finish()),
        },
        Err(err) => {
            log::error!("{:?}", err);
            Ok(HttpResponse::NotFound().finish())
        },
    }
}
pub async fn check_custom_domain(
    session: TypedSession,
    params: web::Query<BasicForm>,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    let tapp = get(&params.name, &user, kubetailor.clone()).await;
    if tapp.name.is_empty() {
        return Ok(HttpResponse::BadRequest().body(format!("Tapp: {} -not found-", tapp.name)));
    };

    if let Some(domain) = tapp.domains.custom {
        let url = url::Url::parse(&format!("https://{}", domain))
            .unwrap()
            .to_string();
        match kubetailor.client.get(url).send().await {
            Ok(response) => match response.status() {
                StatusCode::OK => Ok(HttpResponse::Ok().finish()),
                StatusCode::NOT_FOUND => Ok(HttpResponse::NotFound().finish()),
                _ => Ok(HttpResponse::Created().finish()),
            },
            Err(err) => {
                log::error!("{:?}", err);
                Ok(HttpResponse::NotFound().finish())
            },
        }
    } else {
        Ok(HttpResponse::Created().finish())
    }
}

pub async fn deploying(
    hb: web::Data<Handlebars<'_>>,
    session: TypedSession,
    params: web::Query<BasicForm>,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    let tapp = get(&params.name, &user, kubetailor.clone()).await;
    if tapp.name.is_empty() {
        return Ok(HttpResponse::NotFound().body(format!("Tapp: {} -not found-", tapp.name)));
    };
    let data = json!({
        "title": "Deploying tapp",
        "head": format!("Deploying Tapp: {}", params.name),
        "subtitle": "Processing changes",
        "show_back": true,
        "home": true,
        "tapp": tapp,
        "user": user,
    });
    let body = hb.render("loading", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
}
pub async fn delete_form(
    hb: web::Data<Handlebars<'_>>,
    session: TypedSession,
    params: web::Query<BasicForm>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    let data = json!({
        "title": "Destroying tapp",
        "head": format!("Destroying Tapp: {}", params.name),
        "subtitle": "This action CANNOT be reverted. Proceed?",
        "tapp_name": params.name,
        "loading": params.loading.unwrap_or(false),
        "user": user,
    });
    let body = hb.render("forms/delete-confirm", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
}

pub async fn delete(
    form: web::Form<BasicForm>,
    session: TypedSession,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, ApiError> {
    let user = if let Some(email) = session.get_user().map_err(e500).unwrap() {
        email
    } else {
        return Ok(see_other("/login"));
    };
    //Check if owner.
    let items: Vec<String> = kubetailor
        .client
        .get(format!(
            "{}/list?owner={}&filter=name",
            kubetailor.url, user
        ))
        .send()
        .await
        .unwrap()
        .json::<Vec<String>>()
        .await
        .unwrap();

    if items.into_iter().any(|name| name == form.name) {
        kubetailor
            .client
            .delete(&format!("{}/{}", kubetailor.url, form.name))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        Ok(see_other(&format!(
            "/dashboard/delete?name={}&loading=true",
            form.name
        )))
    } else {
        Ok(HttpResponse::NotFound().body(format!("Tapp {} not found", form.name)))
    }
}
