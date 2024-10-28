use actix_web::{web::Data, App, HttpServer};
use kubetailor::kube::{Client, Config as KubeConfig};

mod config;
mod deployment;
mod error;
mod git;
mod health;
mod ingress;
mod quickwit;
mod routes;
mod tapp;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let config: config::Config = config::Config::load().expect("unable to read config file");
    let server_addr = format!("{}:{}", config.server.addr, config.server.port);
    std::env::set_var("RUST_LOG", config.server.log_level);
    let kube_config = KubeConfig::infer()
        .await
        .expect("Failed to infer kube config");

    HttpServer::new(move || {
        let kubetailor =
            Client::try_from(kube_config.clone()).expect("Failed to create kube client");
        let quickwit = quickwit::Client::new(format!(
            "{}/api/{}/{}/search",
            config.quickwit.url, config.quickwit.api_version, config.quickwit.index
        ));
        App::new()
            .app_data(Data::new(kubetailor))
            .app_data(Data::new(quickwit))
            .app_data(Data::new(config.kubetailor.clone()))
            .service(routes::create)
            .service(routes::list)
            .service(routes::health)
            .service(routes::get)
            .service(routes::logs)
            .service(routes::update)
            .service(routes::delete)
    })
    .bind(server_addr)?
    .run()
    .await
}
