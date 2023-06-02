use std::str::FromStr;

use actix_web::{
    delete, get, post, put, web,
    web::{Data, Json},
    HttpResponse, Responder,
};
use kube::{
    api::{Api as KubeApi, DeleteParams, ListParams, PostParams},
    Client, ResourceExt,
};
use kubetailor::prelude::*;
use log::info;
use serde::{Serialize, Deserialize};

use crate::{config::Kubetailor, tapp::TappRequest};

#[derive(Deserialize)]
pub struct BasicParams {
    pub owner: String,
    pub group: Option<String>,
    pub filter: Option<String>,
}

#[derive(Deserialize)]
enum ListFilter {
    Name,
    Spec,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ListOutput {
    Spec(Vec<TailoredAppSpec>),
    Name(Vec<String>),
}

#[derive(Deserialize)]
enum TappFilter {
    Container,
    Ingress,
    Certificate,
    Config,
    Secrets,
}

impl FromStr for ListFilter {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "name" => Ok(ListFilter::Name),
            "spec" => Ok(ListFilter::Spec),
            _ => Err(()),
        }
    }
}
#[post("/")]
pub async fn create(
    client: Data<Client>,
    mut payload: Json<TappRequest>,
    kubetailor: Data<Kubetailor>,
) -> impl Responder {
    let namespace: String = kubetailor.namespace.clone();

    payload.kubetailor = kubetailor.as_ref().clone();

    let app: TailoredApp = match TailoredApp::try_from(payload.into_inner()) {
        Ok(k) => k,
        Err(e) => {
            return HttpResponse::InternalServerError().body(e.to_string());
        },
    };
    info!("Creating TailoredApp: {app:?}");

    let api: KubeApi<TailoredApp> = KubeApi::namespaced(
        client.as_ref().clone(),
        &app.namespace().unwrap_or(namespace),
    );

    match api.create(&PostParams::default(), &app).await {
        Ok(_) => HttpResponse::Created().body(format!("Created TailoredApp: {}", app.name_any())),
        Err(e) => HttpResponse::BadRequest().body(e.to_string()),
    }
}

#[put("/")]
pub async fn update(
    client: Data<Client>,
    mut payload: Json<TappRequest>,
    kubetailor: Data<Kubetailor>,
) -> impl Responder {
    let namespace: String = kubetailor.namespace.clone();

    //Add config to payload
    payload.kubetailor = kubetailor.as_ref().clone();
    let mut app: TailoredApp = match TailoredApp::try_from(payload.into_inner()) {
        Ok(k) => k,
        Err(e) => {
            return HttpResponse::InternalServerError().body(e.to_string());
        },
    };
    let api: KubeApi<TailoredApp> = KubeApi::namespaced(
        client.get_ref().clone(),
        &app.namespace().unwrap_or(namespace),
    );

    let app_name = &app.metadata.name.as_ref().unwrap();
    let resource_version = match api.get(app_name).await {
        Ok(manifest) => manifest.resource_version(),
        Err(_) => {
            return HttpResponse::NotFound().body(format!("TailoredApp '{}' not found", app_name));
        },
    };
    app.metadata.resource_version = resource_version;
    match api.replace(app_name, &PostParams::default(), &app).await {
        Ok(_) => HttpResponse::Ok().body(format!("Updated TailoredApp: {}", app_name)),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn fetch_tapps(
    client: &Client,
    owner: &str,
    kubetailor: &Kubetailor,
) -> Option<Vec<TailoredApp>> {
    let api: KubeApi<TailoredApp> =
        KubeApi::namespaced(client.to_owned(), kubetailor.namespace.as_str());
    let owner = owner.clone().replace('@', "-");
    let list_params = ListParams::default().labels(&format!("owner={}", owner));

    match api.list(&list_params).await {
        Ok(tapps) => Some(tapps.items),
        Err(_) => None,
    }
}

#[get("/list")]
pub async fn list(
    client: Data<Client>,
    params: web::Query<BasicParams>,
    kubetailor: Data<Kubetailor>,
) -> impl Responder {
    let items = fetch_tapps(&client, &params.owner, &kubetailor).await;

    match items {
        Some(tapps) => {
            let list = match ListFilter::from_str(params.filter.as_deref().unwrap_or("spec")) {
                Ok(ListFilter::Spec) => ListOutput::Spec(tapps.iter().map(|tapp| tapp.spec.clone()).collect()),
                Ok(ListFilter::Name) => ListOutput::Name(tapps.iter().map(|tapp| tapp.metadata.name.clone().unwrap()).collect()),
                Err(_) => return HttpResponse::BadRequest().body("Invalid filter value"),
            };

            HttpResponse::Ok().json(list)
        },
        None => HttpResponse::NotFound().body("No TailoredApps found"),
    }
}

#[delete("/{app_name}")]
pub async fn delete(
    client: Data<Client>,
    app_name: web::Path<String>,
    params: web::Query<BasicParams>,
    kubetailor: Data<Kubetailor>,
) -> impl Responder {
    let items = fetch_tapps(&client, &params.owner, &kubetailor).await;

    match items {
        Some(tapps) => {
            if let Some(tapp) = tapps.into_iter().find(|tapp| tapp.metadata.name.as_ref() == Some(&app_name.to_string())) {
                let api: KubeApi<TailoredApp> =
                    KubeApi::namespaced(client.get_ref().clone(), kubetailor.namespace.as_str());

                match api.delete(&tapp.metadata.name.unwrap(), &DeleteParams::default()).await {
                    Ok(_) => HttpResponse::Ok().body(format!("Deleted TailoredApp: {}", app_name)),
                    Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
                }
            } else {
                HttpResponse::NotFound().body(format!("TailoredApp '{}' not found", app_name))
            }
        },
        None => HttpResponse::NotFound().body("No TailoredApps found"),
    }
}

#[get("/{app_name}")]
pub async fn get(
    client: Data<Client>,
    app_name: web::Path<String>,
    params: web::Query<BasicParams>,
    kubetailor: Data<Kubetailor>,
) -> impl Responder {
    let items = fetch_tapps(&client, &params.owner, &kubetailor).await;

    match items {
        Some(tapps) => {
            if let Some(tapp) = tapps.into_iter().find(|tapp| tapp.metadata.name.as_ref() == Some(&app_name.to_string())) {
                let tapp = TappRequest::try_from(tapp).unwrap();
                HttpResponse::Ok().json(tapp)
            } else {
                HttpResponse::NotFound().body("No TailoredApp found with the specified name")
            }
        },
        None => HttpResponse::NotFound().body("No TailoredApps found"),
    }
}
