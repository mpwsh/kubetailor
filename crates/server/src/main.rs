use actix_web::{web::Data, App, HttpServer};
use kube::{Client, Config as KubeConfig};

mod config;
mod deployment;
mod error;
mod ingress;
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
        let client = Client::try_from(kube_config.clone()).expect("Failed to create kube client");
        App::new()
            .app_data(Data::new(client))
            .app_data(Data::new(config.kubetailor.clone()))
            .service(routes::create)
            .service(routes::list)
            .service(routes::get)
            .service(routes::update)
            .service(routes::delete)
    })
    .bind(server_addr)?
    .run()
    .await
}
