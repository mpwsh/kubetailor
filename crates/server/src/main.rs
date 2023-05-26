use std::str::FromStr;

use actix_web::{
    delete, get, post, put, web,
    web::{Data, Json},
    App, HttpResponse, HttpServer, Responder,
};
use config::{Config, KubetailorConfig, TailoredAppConfig};
use kube::{
    api::{Api as KubeApi, DeleteParams, ListParams, ObjectList, PostParams},
    Client, Config as KubeConfig, ResourceExt,
};
use kubetailor::prelude::*;
use log::info;
mod config;

#[derive(Deserialize)]
pub struct BasicParams {
    pub owner: Option<String>,
    pub filter: Option<String>,
}

#[derive(Deserialize)]
enum ListFilter {
    Name,
    Metadata,
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
            "metadata" => Ok(ListFilter::Metadata),
            _ => Err(()),
        }
    }
}
#[post("/")]
async fn create(
    client: Data<Client>,
    mut payload: Json<TailoredAppConfig>,
    kubetailor: Data<KubetailorConfig>,
) -> impl Responder {
    let namespace: String = kubetailor.namespace.clone();

    //Add config to payload
    payload.kubetailor = kubetailor.as_ref().clone();

    let app: TailoredApp = match TailoredApp::try_from(payload.into_inner()) {
        Ok(k) => k,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Error parsing TailoredApp Config: {}", e));
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
async fn update(
    client: Data<Client>,
    mut payload: Json<TailoredAppConfig>,
    kubetailor: Data<KubetailorConfig>,
) -> impl Responder {
    let namespace: String = kubetailor.namespace.clone();

    //Add config to payload
    payload.kubetailor = kubetailor.as_ref().clone();
    let mut app: TailoredApp = match TailoredApp::try_from(payload.into_inner()) {
        Ok(k) => k,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("Error parsing TailoredApp Config: {}", e));
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
        Err(e) => {
            HttpResponse::InternalServerError().body(format!("Error updating TailoredApp: {}", e))
        },
    }
}
#[get("/list")]
async fn list(
    client: Data<Client>,
    params: web::Query<BasicParams>,
    kubetailor: Data<KubetailorConfig>,
) -> impl Responder {
    let api: KubeApi<TailoredApp> =
        KubeApi::namespaced(client.get_ref().clone(), kubetailor.namespace.as_str());
    let owner = params.owner.clone().unwrap().replace('@', "-");
    let tapps = ListParams::default().labels(&format!("owner={owner}"));
    let items: ObjectList<TailoredApp> = api
        .list(&tapps)
        .await
        .expect("unable to list tailored apps");

    let list = match ListFilter::from_str(&params.filter.clone().unwrap()) {
        Ok(ListFilter::Metadata) => items.iter().map(|tapp| tapp.name_any()).collect::<Vec<_>>(),
        Ok(ListFilter::Name) => items.iter().map(|tapp| tapp.name_any()).collect(),
        Err(_) => {
            // Handle the error case here, e.g., return an error response
            return HttpResponse::BadRequest().body("Invalid filter value");
        },
    };

    HttpResponse::Ok().json(list)
}

#[delete("/{app_name}")]
async fn delete(
    client: Data<Client>,
    app_name: web::Path<String>,
    kubetailor: Data<KubetailorConfig>,
) -> impl Responder {
    let api: KubeApi<TailoredApp> =
        KubeApi::namespaced(client.get_ref().clone(), kubetailor.namespace.as_str());

    if api.get(app_name.as_str()).await.is_err() {
        return HttpResponse::NotFound().body(format!("TailoredApp '{}' not found", app_name));
    }

    match api
        .delete(app_name.as_str(), &DeleteParams::default())
        .await
    {
        Ok(_) => HttpResponse::Ok().body(format!("Deleted TailoredApp: {}", app_name)),
        Err(e) => {
            HttpResponse::InternalServerError().body(format!("Error deleting TailoredApp: {}", e))
        },
    };

    HttpResponse::Ok().body(format!("Deleted TailoredApp '{}'", app_name))
}
#[get("/{app_name}")]
async fn get(
    client: Data<Client>,
    app_name: web::Path<String>,
    kubetailor: Data<KubetailorConfig>,
) -> impl Responder {
    let api: KubeApi<TailoredApp> =
        KubeApi::namespaced(client.get_ref().clone(), kubetailor.namespace.as_str());

    match api.get(app_name.as_str()).await {
        Ok(tapp) => {
            let tapp = TailoredAppConfig::try_from(tapp).unwrap();
            HttpResponse::Ok().json(tapp)
        },
        Err(e) => HttpResponse::InternalServerError().body(format!(
            "TailoredApp '{}' not found. Error: {}",
            app_name, e
        )),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let config: Config = config::Config::load().expect("unable to read config file");
    let server_addr = format!("{}:{}", config.server.addr, config.server.port);
    std::env::set_var("RUST_LOG", config.server.log_level);
    let kube_config = KubeConfig::infer()
        .await
        .expect("Failed to infer kube config");

    HttpServer::new(move || {
        let client = Client::try_from(kube_config.clone()).expect("Failed to create kube client");
        App::new()
            .app_data(Data::new(client))
            .app_data(Data::new(config.kubetailor.clone()))
            .service(create)
            .service(list)
            .service(get)
            .service(update)
            .service(delete)
    })
    .bind(server_addr)?
    .run()
    .await
}
