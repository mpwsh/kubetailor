use serde::{Deserialize, Serialize};
#[derive(Clone)]
pub struct Client {
    pub client: reqwest::Client,
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Logs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hits: Option<Vec<LogBody>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LogBody {
    pub body: LogMessage,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LogMessage {
    pub message: String,
}

impl Client {
    pub fn new(url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            url,
        }
    }
}
