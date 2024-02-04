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
    session: TypedSession,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };

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
        "title": "Dashboard",
        "head": "Tapps",
        "deployments": items,
        "home": true,
        "action": "New",
        "user": user,
    });
    let body = hb.render("home", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
}

pub async fn view(
    hb: web::Data<Handlebars<'_>>,
    params: web::Query<BasicForm>,
    session: TypedSession,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    let tapp = tapp::get(&params.name, &user, kubetailor.clone()).await;
    let data = json!({
        "title": format!("{} Details", params.name),
        "head": format!("{} Details", params.name),
        "show_back": true,
        "view": true,
        "action": "Edit",
        "user": user,
        "tapp": tapp,
    });
    let body = hb.render("view", &data).unwrap();
    if tapp.name.is_empty() {
        Ok(HttpResponse::NotFound().body(""))
    } else {
        Ok(HttpResponse::Ok().body(body))
    }
}

pub async fn check_shared_domain(
    session: TypedSession,
    params: web::Query<BasicForm>,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
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
    session: TypedSession,
    params: web::Query<BasicForm>,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
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
    session: TypedSession,
    params: web::Query<BasicForm>,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    let tapp = tapp::get(&params.name, &user, kubetailor.clone()).await;
    if tapp.name.is_empty() {
        return Ok(HttpResponse::NotFound().body(format!("Tapp: {} -not found-", tapp.name)));
    };
    let data = json!({
        "title": "Deploying tapp",
        "head": format!("Deploying Tapp: {}", params.name),
        "subtitle": "Processing changes",
        "show_back": true,
        "home": true,
        "tapp": tapp,
        "user": user,
    });
    let body = hb.render("loading", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
}
