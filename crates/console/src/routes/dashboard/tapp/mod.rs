pub mod delete;
pub mod edit;
pub mod logs;
pub mod new;

use crate::routes::prelude::*;

pub async fn get(tapp_name: &str, owner: &str, kubetailor: web::Data<Kubetailor>) -> TappConfig {
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
