pub mod delete;
pub mod edit;
pub mod health;
pub mod new;
pub mod restart;

use crate::routes::prelude::*;

#[derive(Deserialize)]
pub struct BasicForm {
    pub name: String,
    pub loading: Option<bool>,
}
pub async fn page(
    hb: web::Data<Handlebars<'_>>,
    kubetailor: web::Data<Kubetailor>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();

    let items: Vec<String> = kubetailor
        .client
        .get(format!(
            "{}/list?owner={}&filter=name",
            kubetailor.url, user
        ))
        .send()
        .await
        .unwrap()
        .json::<Vec<String>>()
        .await
        .unwrap();

    let action = Action::new("New").url("/deployments/new");
    let data = json!({
        "initial": !req.is_htmx(),
        "title": "Deployments",
        "head": "Tapps",
        "deployments": items,
        "action": action,
        "user": user,
    });
    let body = hb.render("deployments/main", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
}

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

pub async fn view(
    hb: web::Data<Handlebars<'_>>,
    params: web::Query<BasicForm>,
    req: HttpRequest,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();

    let tapp = deployments::get(&params.name, &user, kubetailor.clone()).await;

    let action = Action::new("Edit").url(&format!("/deployments/edit?name={}", tapp.name));

    let data = json!({
        "initial":  !req.is_htmx(),
        "title": format!("{} Details", params.name),
        "action": action,
        "user": user,
        "tapp": tapp,
    });
    let body = hb.render("deployments/view", &data).unwrap();
    if tapp.name.is_empty() {
        Ok(HttpResponse::NotFound().body(""))
    } else {
        Ok(HttpResponse::Ok().body(body))
    }
}

pub async fn list(
    req: HttpRequest,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();

    let items: Vec<String> = kubetailor
        .client
        .get(format!(
            "{}/list?owner={}&filter=name",
            kubetailor.url, user
        ))
        .send()
        .await
        .unwrap()
        .json::<Vec<String>>()
        .await
        .unwrap();

    let data = json!({
        "deployments": items,
    });
    Ok(HttpResponse::Ok().json(data))
}

pub async fn deploying(
    hb: web::Data<Handlebars<'_>>,
    params: web::Query<BasicForm>,
    kubetailor: web::Data<Kubetailor>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();
    let is_htmx = req.headers().contains_key("HX-Request");
    let template = if is_htmx {
        "deployments/status"
    } else {
        "deployments/deploying"
    };

    let status = health::get(params.name.clone(), user.clone(), kubetailor.clone()).await?;

    let action = Action::new("Confirm").url("/dashboard");
    let is_htmx = !req.is_htmx();
    let data = json!({
        "initial": is_htmx,
        "title": "Deploying tapp",
        "head": format!("Deploying Tapp: {}", params.name),
        "subtitle": "Processing changes",
        "show_back": true,
        "tapp_name": params.name.clone(),
        "action": action,
        "status": status,
        "user": user,
    });
    let body = hb.render(template, &data).unwrap();
    Ok(HttpResponse::Ok().body(body))
}
