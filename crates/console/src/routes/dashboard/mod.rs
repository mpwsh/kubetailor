use crate::routes::prelude::*;

pub mod tapp;

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

    let is_htmx = req.headers().contains_key("HX-Request");

    let template = if is_htmx { "home" } else { "dashboard" };

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

    let action = Action::new("New").url("/dashboard/tapp/new");
    let data = json!({
        "title": "Dashboard",
        "head": "Tapps",
        "deployments": items,
        "action": action,
        "user": user,
    });
    let body = hb.render(template, &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
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

    let is_htmx = req.headers().contains_key("HX-Request");

    let template = if is_htmx { "summary" } else { "view" };

    let tapp = tapp::get(&params.name, &user, kubetailor.clone()).await;

    let action = Action::new("Edit").url(&format!("/dashboard/tapp/edit?name={}", tapp.name));

    let data = json!({
        "title": format!("{} Details", params.name),
        "return_url": "/dashboard",
        "action": action,
        "user": user,
        "tapp": tapp,
    });
    let body = hb.render(template, &data).unwrap();
    if tapp.name.is_empty() {
        Ok(HttpResponse::NotFound().body(""))
    } else {
        Ok(HttpResponse::Ok().body(body))
    }
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
    let template = if is_htmx { "status" } else { "deploying" };
    let status = tapp::health::get(&params.name, &user, kubetailor.clone()).await;
    let action = Action::new("Confirm").url("/dashboard");

    let data = json!({
        "title": "Deploying tapp",
        "head": format!("Deploying Tapp: {}", params.name),
        "subtitle": "Processing changes",
        "show_back": true,
        "tapp_name": params.name,
        "action": action,
        "status": status,
        "user": user,
    });

    let body = hb.render(template, &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
}
