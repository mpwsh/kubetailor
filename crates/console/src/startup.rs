use std::net::TcpListener;

use actix_files as fs;
use actix_session::{config::PersistentSession, storage::RedisSessionStore, SessionMiddleware};
use actix_web::{
    cookie::{self, Key},
    dev::Server,
    web::{self, resource},
    App, HttpServer, Result,
};
use actix_web_flash_messages::{storage::CookieMessageStore, FlashMessagesFramework};
use actix_web_lab::middleware::from_fn;
use handlebars::Handlebars;
use portier::Client;
use secrecy::{ExposeSecret, Secret};
use tracing_actix_web::TracingLogger;

use crate::{
    authentication::reject_anonymous_users,
    config::{Kubetailor, Settings},
    errors::error_handlers,
    routes::{
        check_custom_domain, check_shared_domain, claim, dashboard, delete, delete_form, deploying,
        edit, edit_form, health, login, login_form, logout, new, new_form, view, whoami,
    },
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

    let mut handlebars = Handlebars::new();
    handlebars.register_templates_directory(".hbs", "./templates")?;

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
            .wrap(error_handlers())
            .wrap(TracingLogger::default())
            .app_data(web::Data::new(client.clone()))
            .app_data(web::Data::new(kubetailor.clone()))
            .app_data(web::Data::new(handlebars.clone()))
            .route("/", web::get().to(login_form))
            .route("/health", web::get().to(health))
            .service(
                resource("/login")
                    .route(web::get().to(login_form))
                    .route(web::post().to(login)),
            )
            .service(
                web::scope("/dashboard")
                    .wrap(from_fn(reject_anonymous_users))
                    .route("/", web::get().to(dashboard))
                    .route("", web::get().to(dashboard))
                    .route("/loading", web::get().to(deploying))
                    .route("/view", web::get().to(view))
                    .route("/custom_domain", web::get().to(check_custom_domain))
                    .route("/shared_domain", web::get().to(check_shared_domain))
                    .route("/edit", web::get().to(edit_form))
                    .service(
                        resource("/new")
                            .route(web::get().to(new_form))
                            .route(web::post().to(new)),
                    )
                    .service(
                        resource("/edit")
                            .route(web::get().to(edit_form))
                            .route(web::post().to(edit)),
                    )
                    .service(
                        resource("/delete")
                            .route(web::get().to(delete_form))
                            .route(web::post().to(delete)),
                    ),
            )
            .service(resource(claim_path).route(web::post().to(claim)))
            .service(
                resource("/logout")
                    .route(web::get().to(logout))
                    .route(web::post().to(logout)),
            )
            .service(resource("/whoami").route(web::get().to(whoami)))
            .service(fs::Files::new("/static/", "./static/"))
    })
    .listen(listener)?
    .run();

    Ok(server)
}
