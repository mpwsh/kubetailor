use std::str::FromStr;

use actix_web::{
    delete, get, post, put, web,
    web::{Data, Json},
    HttpResponse, Responder,
};
use kubetailor::{
    //k8s_openapi::api::{apps::v1::Deployment, core::v1::Pod, networking::v1::Ingress},
    kube::{
        api::{Api as KubeApi, DeleteParams, ListParams, PostParams},
        core::NamespaceResourceScope,
        Client, ResourceExt,
    },
    prelude::*,
};
use log::info;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{config::Kubetailor, health::Health, quickwit, tapp::TappRequest};

#[derive(Deserialize)]
pub struct BasicParams {
    pub owner: String,
    pub group: Option<String>,
    pub filter: Option<String>,
}

#[derive(Deserialize)]
pub struct LogsParams {
    pub owner: String,
    pub query: Option<String>,
    pub from_ts: Option<String>,
    pub to_ts: Option<String>,
}

#[derive(Deserialize)]
enum ListFilter {
    Name,
    Spec,
    Status,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ListOutput<T> {
    Spec(Vec<T>),
    Name(Vec<String>),
}

impl FromStr for ListFilter {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "name" => Ok(ListFilter::Name),
            "spec" => Ok(ListFilter::Spec),
            "status" => Ok(ListFilter::Status),
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

pub async fn fetch_resources<T>(client: &Client, owner: &str, namespace: &str) -> Option<Vec<T>>
where
    T: Clone
        + DeserializeOwned
        + Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + ResourceExt
        + std::fmt::Debug,
{
    let api: KubeApi<T> = KubeApi::namespaced(client.to_owned(), namespace);
    let owner = owner.replace('@', "-");
    let list_params = ListParams::default().labels(&format!("owner={owner}"));

    match api.list(&list_params).await {
        Ok(res) => Some(res.items),
        Err(_) => None,
    }
}

async fn fetch_resource<T>(
    client: &Client,
    owner: &str,
    namespace: &str,
    tapp_name: &str,
) -> Option<Vec<T>>
where
    T: Clone
        + DeserializeOwned
        + Resource<DynamicType = (), Scope = NamespaceResourceScope>
        + ResourceExt
        + std::fmt::Debug,
{
    let api: KubeApi<T> = KubeApi::namespaced(client.to_owned(), namespace);
    let owner = owner.replace('@', "-");
    let list_params = ListParams::default().labels(&format!("owner={owner},tapp={tapp_name}"));

    match api.list(&list_params).await {
        Ok(res) => Some(res.items),
        Err(_) => None,
    }
}

#[get("/list")]
pub async fn list(
    client: Data<Client>,
    params: web::Query<BasicParams>,
    kubetailor: Data<Kubetailor>,
) -> impl Responder {
    let items: Option<Vec<TailoredApp>> =
        fetch_resources(&client, &params.owner, &kubetailor.namespace).await;

    match items {
        Some(tapps) => {
            let results = match ListFilter::from_str(params.filter.as_deref().unwrap_or("spec")) {
                Ok(ListFilter::Spec) => {
                    ListOutput::Spec(tapps.iter().map(|tapp| tapp.spec.clone()).collect())
                },
                Ok(ListFilter::Status) => {
                    ListOutput::Spec(tapps.iter().map(|tapp| tapp.spec.clone()).collect())
                },
                Ok(ListFilter::Name) => ListOutput::Name(
                    tapps
                        .iter()
                        .map(|tapp| tapp.metadata.name.clone().unwrap())
                        .collect(),
                ),
                Err(_) => return HttpResponse::BadRequest().body("Invalid filter value"),
            };

            HttpResponse::Ok().json(results)
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
    let items: Option<Vec<TailoredApp>> =
        fetch_resources(&client, &params.owner, &kubetailor.namespace).await;

    match items {
        Some(tapps) => {
            if let Some(tapp) = tapps
                .into_iter()
                .find(|tapp| tapp.metadata.name.as_ref() == Some(&app_name.to_string()))
            {
                let api: KubeApi<TailoredApp> =
                    KubeApi::namespaced(client.get_ref().clone(), kubetailor.namespace.as_str());

                match api
                    .delete(&tapp.metadata.name.unwrap(), &DeleteParams::default())
                    .await
                {
                    Ok(_) => HttpResponse::Ok().body(format!("Deleted TailoredApp: {app_name}")),
                    Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
                }
            } else {
                HttpResponse::NotFound().body(format!("TailoredApp {app_name} not found"))
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
    let items: Option<Vec<TailoredApp>> =
        fetch_resources(&client, &params.owner, &kubetailor.namespace).await;

    match items {
        Some(tapps) => {
            if let Some(tapp) = tapps
                .into_iter()
                .find(|tapp| tapp.metadata.name.as_ref() == Some(&app_name.to_string()))
            {
                let tapp = TappRequest::try_from(tapp).unwrap();
                HttpResponse::Ok().json(tapp)
            } else {
                HttpResponse::NotFound().body("No TailoredApp found with the specified name")
            }
        },
        None => HttpResponse::NotFound().body("No TailoredApps found"),
    }
}

#[get("/{tapp_name}/health")]
pub async fn health(
    client: Data<Client>,
    tapp_name: web::Path<String>,
    params: web::Query<BasicParams>,
    kubetailor: Data<Kubetailor>,
) -> impl Responder {
    let resources = Resources {
        cert: fetch_resource(&client, &params.owner, &kubetailor.namespace, &tapp_name).await,
        pods: fetch_resource(&client, &params.owner, &kubetailor.namespace, &tapp_name).await,
        deployment: fetch_resource(&client, &params.owner, &kubetailor.namespace, &tapp_name).await,
        ingress: fetch_resource(&client, &params.owner, &kubetailor.namespace, &tapp_name).await,
    };
    if let Ok(status) = Health::try_from(resources) {
        HttpResponse::Ok().json(status)
    } else {
        HttpResponse::Ok().body(format!("Unable to get status of {tapp_name}"))
    }
}

#[get("/{app_name}/logs")]
pub async fn logs(
    client: Data<Client>,
    app_name: web::Path<String>,
    params: web::Query<LogsParams>,
    kubetailor: Data<Kubetailor>,
    quickwit: Data<quickwit::Client>,
) -> impl Responder {
    let items: Option<Vec<TailoredApp>> =
        fetch_resources(&client, &params.owner, &kubetailor.namespace).await;
    let owner = params.owner.replace('@', "-");

    match items {
        Some(tapps) => {
            if let Some(tapp) = tapps
                .into_iter()
                .find(|tapp| tapp.metadata.name.as_ref() == Some(&app_name.to_string()))
            {
                let tapp_name = tapp.metadata.name.unwrap();
                let base_query = format!(
                    "resource_attributes.tapp:{tapp_name} AND resource_attributes.owner:{owner} AND NOT resource_attributes.k8s.container.name:init AND NOT resource_attributes.k8s.container.name:git-sync",
                );
                //Here we query for logs
                let query = if let Some(query) = &params.query {
                    json!({
                        "query":
                            format!(
                                "{base_query} AND body.message:{query}",
                                query = query.replace(' ', "+")
                            )
                    })
                } else {
                    json!({
                    "query": base_query,
                    })
                };

                let logs = quickwit
                    .client
                    .post(&quickwit.url)
                    .json(&query)
                    .send()
                    .await
                    .unwrap()
                    .json::<quickwit::Logs>()
                    .await
                    .unwrap();
                let res: Vec<String> = logs
                    .hits
                    .unwrap_or_default()
                    .iter()
                    .map(|log| log.body.message.clone())
                    .collect();
                HttpResponse::Ok().json(res)
            } else {
                HttpResponse::NotFound().body("No TailoredApp found with the specified name")
            }
        },
        None => HttpResponse::NotFound().body("No TailoredApps found"),
    }
}
