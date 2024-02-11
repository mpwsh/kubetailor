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

    let action = json!({
        "name": "New",
        "url": "/dashboard/tapp/new",
    });
    let data = json!({
        "title": "Dashboard",
        "head": "Tapps",
        "deployments": items,
        "action": action,
        "user": user,
    });
    let body = hb.render("dashboard", &data).unwrap();

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

    // Detect HTMX request
    let is_htmx = req.headers().contains_key("HX-Request");

    // Decide which template to render based on HTMX request
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

pub async fn check_shared_domain(
    params: web::Query<BasicForm>,
    kubetailor: web::Data<Kubetailor>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();

    let tapp = tapp::get(&params.name, &user, kubetailor.clone()).await;
    if tapp.name.is_empty() {
        return Ok(HttpResponse::BadRequest().body(format!("Tapp: {} -not found-", tapp.name)));
    };

    let url = url::Url::parse(&format!("https://{}", tapp.domains.shared))
        .unwrap()
        .to_string();
    log::error!("Using URL: {}", tapp.domains.shared);
    match kubetailor.client.get(url).send().await {
        Ok(response) => match response.status() {
            StatusCode::OK => Ok(HttpResponse::Ok().finish()),
            StatusCode::NOT_FOUND => Ok(HttpResponse::NotFound().finish()),
            _ => Ok(HttpResponse::Created().finish()),
        },
        Err(err) => {
            log::error!("{:?}", err);
            Ok(HttpResponse::NotFound().finish())
        },
    }
}
pub async fn check_custom_domain(
    params: web::Query<BasicForm>,
    kubetailor: web::Data<Kubetailor>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();
    let tapp = tapp::get(&params.name, &user, kubetailor.clone()).await;
    if tapp.name.is_empty() {
        return Ok(HttpResponse::BadRequest().body(format!("Tapp: {} -not found-", tapp.name)));
    };

    if let Some(domain) = tapp.domains.custom {
        let url = url::Url::parse(&format!("https://{}", domain))
            .unwrap()
            .to_string();
        match kubetailor.client.get(url).send().await {
            Ok(response) => match response.status() {
                StatusCode::OK => Ok(HttpResponse::Ok().finish()),
                StatusCode::NOT_FOUND => Ok(HttpResponse::NotFound().finish()),
                _ => Ok(HttpResponse::Created().finish()),
            },
            Err(err) => {
                log::error!("{:?}", err);
                Ok(HttpResponse::NotFound().finish())
            },
        }
    } else {
        Ok(HttpResponse::Created().finish())
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

    let tapp = tapp::get(&params.name, &user, kubetailor.clone()).await;
    if tapp.name.is_empty() {
        return Ok(HttpResponse::NotFound().body(format!("Tapp: {} -not found-", tapp.name)));
    };

    let data = json!({
        "title": "Deploying tapp",
        "head": format!("Deploying Tapp: {}", params.name),
        "subtitle": "Processing changes",
        "show_back": true,
        "tapp": tapp,
        "user": user,
    });
    let body = hb.render("loading", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
}
