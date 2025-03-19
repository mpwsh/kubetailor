use std::net::TcpListener;

use actix_files as fs;
use actix_session::{config::PersistentSession, storage::RedisSessionStore, SessionMiddleware};
use actix_web::{
    cookie::{self, Key},
    dev::Server,
    middleware::from_fn,
    web::{self, resource},
    App, HttpServer, Result,
};
use actix_web_flash_messages::{storage::CookieMessageStore, FlashMessagesFramework};

use portier::Client;
use secrecy::{ExposeSecret, Secret};
use tracing_actix_web::TracingLogger;

use crate::{
    authentication::reject_anonymous_users,
    config::{Kubetailor, Settings},
    errors::{error_handlers, handle_not_found},
    handlebars,
    routes::{authenticate, claim, dashboard::*, error, login, logout, whoami},
};

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(config: Settings) -> Result<Self, anyhow::Error> {
        let address = format!("{}:{}", config.application.host, config.application.port);
        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(
            listener,
            config.application.base_url,
            config.application.hmac_secret,
            config.redis_dsn,
            config.portier_url,
            config.kubetailor_url,
            config.application.session_ttl,
        )
        .await?;

        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub struct ApplicationBaseUrl(pub String);

pub async fn run(
    listener: TcpListener,
    base_url: String,
    hmac_secret: Secret<String>,
    redis_dsn: Secret<String>,
    portier_url: Secret<String>,
    kubetailor_url: Secret<String>,
    session_ttl: i64,
) -> Result<Server, anyhow::Error> {
    let claim_path = "/claim";
    let redirect_url = format!("{base_url}{claim_path}").parse().unwrap();

    let handlebars = handlebars::create_handlebars()?;

    let client = Client::builder(redirect_url)
        .broker(portier_url.expose_secret().parse().unwrap())
        .build()
        .unwrap();

    let secret_key = Key::from(hmac_secret.expose_secret().as_bytes());
    let message_store = CookieMessageStore::builder(secret_key.clone()).build();
    let message_framework = FlashMessagesFramework::builder(message_store).build();
    let redis_store = RedisSessionStore::new(redis_dsn.expose_secret()).await?;
    let kt_client = reqwest::Client::builder()
        .user_agent(APP_USER_AGENT)
        .build()?;

    let kubetailor = Kubetailor {
        client: kt_client,
        url: kubetailor_url.expose_secret().clone(),
    };
    let server = HttpServer::new(move || {
        App::new()
            .wrap(error_handlers())
            .wrap(message_framework.clone())
            .wrap(
                SessionMiddleware::builder(redis_store.clone(), secret_key.clone())
                    .cookie_secure(false)
                    .session_lifecycle(
                        PersistentSession::default()
                            .session_ttl(cookie::time::Duration::hours(session_ttl)),
                    )
                    .build(),
            )
            .wrap(TracingLogger::default())
            .app_data(web::Data::new(client.clone()))
            .app_data(web::Data::new(kubetailor.clone()))
            .app_data(web::Data::new(handlebars.clone()))
            .service(fs::Files::new("/web/static/", "./web/static/"))
            .service(
                resource("/login")
                    .route(web::get().to(login))
                    .route(web::post().to(authenticate)),
            )
            .service(resource(claim_path).route(web::post().to(claim)))
           // .service(
           //     resource("/logout")
            .route("/logout", web::get().to(logout))
                    //.route(web::post().to(logout)),
           // )
            .service(
                web::scope("")
                    .wrap(from_fn(reject_anonymous_users))
                    .route("/", web::get().to(home::page))
                    .route("/logs", web::get().to(logs::page))
                    .route("/whoami", web::get().to(whoami))
                    .route("/error", web::get().to(error::page))
                    .service(
                        web::scope("/deployments")

                            .route("/", web::get().to(deployments::page))
                            .route("", web::get().to(deployments::page))
                            .route("/deploying", web::get().to(deployments::deploying))
                            .route("/view", web::get().to(deployments::view))
                            .route("/list", web::get().to(deployments::list))
                            .route("/health", web::get().to(deployments::health::handler))
                            .service(
                                resource("/new")
                                    .route(web::get().to(deployments::new::page))
                                    .route(web::post().to(deployments::new::form)),
                            )
                            .service(
                                resource("/edit")
                                    .route(web::get().to(deployments::edit::page))
                                    .route(web::post().to(deployments::edit::form)),
                            )
                            .service(
                                resource("/delete")
                                    .route(web::get().to(deployments::delete::page))
                                    .route(web::post().to(deployments::delete::form)),
                            )
                            .service(
                                resource("/restart")
                                    .route(web::get().to(deployments::restart::page))
                                    .route(web::post().to(deployments::restart::form)),
                            )
                        ,
                    ),
            )
            .default_service(web::route().to(handle_not_found))
    })
    .listen(listener)?
    .run();

    Ok(server)
}
