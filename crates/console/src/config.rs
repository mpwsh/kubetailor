use std::convert::{TryFrom, TryInto};

use secrecy::Secret;
use serde_aux::field_attributes::deserialize_number_from_string;

#[derive(serde::Deserialize, Clone)]
pub struct Settings {
    pub application: ApplicationSettings,
    pub redis_dsn: Secret<String>,
    pub portier_url: Secret<String>,
    pub kubetailor_url: Secret<String>,
}

#[derive(serde::Deserialize, Clone)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub base_url: String,
    pub hmac_secret: Secret<String>,
    pub session_ttl: i64,
}

#[derive(Clone)]
pub struct Kubetailor {
    pub client: reqwest::Client,
    pub url: String,
}

impl Kubetailor {
    /// The API's public configuration (base domain, node-port range, image allow-list). A
    /// server without the route, or one that is down, yields the defaults with a warning: the
    /// wizard still renders, it just cannot show the domain suffix.
    pub async fn config(&self) -> crate::models::ApiConfig {
        let fetched = async {
            self.client
                .get(format!("{}/config", self.url))
                .send()
                .await?
                .error_for_status()?
                .json::<crate::models::ApiConfig>()
                .await
        }
        .await;
        fetched.unwrap_or_else(|e| {
            log::warn!("kubetailor config unavailable, using defaults: {e}");
            crate::models::ApiConfig::default()
        })
    }

    /// The manifest the API would create for `tapp` (`POST /preview`), as YAML.
    ///
    /// `Ok(Err(message))` is the API refusing the request — the same validation as a deploy,
    /// worth showing to the person. `Err` is the preview itself being unavailable (an older
    /// server, a connection problem): the review can go on without the manifest.
    pub async fn preview(
        &self,
        tapp: &crate::models::TappConfig,
    ) -> Result<Result<String, String>, reqwest::Error> {
        let response = self
            .client
            .post(format!("{}/preview", self.url))
            .json(tapp)
            .send()
            .await?;
        if response.status().is_success() {
            return Ok(Ok(response.text().await?));
        }
        if response.status().is_client_error()
            && response.status() != reqwest::StatusCode::NOT_FOUND
        {
            return Ok(Err(response.text().await?));
        }
        Err(response.error_for_status().unwrap_err())
    }
}
pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine the current directory");
    let configuration_directory = base_path.join("config");
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");
    let environment_filename = format!("{}.yaml", environment.as_str());
    let settings = config::Config::builder()
        .add_source(config::File::from(
            configuration_directory.join("base.yaml"),
        ))
        .add_source(config::File::from(
            configuration_directory.join(environment_filename),
        ))
        .add_source(
            config::Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    settings.try_deserialize::<Settings>()
}

/// The possible runtime environment for our application.
pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}
