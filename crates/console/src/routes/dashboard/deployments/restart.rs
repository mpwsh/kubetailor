use crate::routes::prelude::*;

#[derive(Deserialize)]
pub struct BasicForm {
    pub name: String,
    pub loading: Option<bool>,
}

pub async fn page(
    hb: web::Data<Handlebars<'_>>,
    params: web::Query<BasicForm>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();

    let data = json!({
        "initial": !req.is_htmx(),
        "title": "Restarting deployment",
        "head": format!("Restarting deployment: {}", params.name),
        "subtitle": "This might cause downtime to your application . Proceed?",
        "tapp_name": params.name,
        "loading": params.loading.unwrap_or(false),
        "user": user,
    });
    let body = hb.render("deployments/restart", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
}

pub async fn form(
    form: web::Form<BasicForm>,
    kubetailor: web::Data<Kubetailor>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();

    //Check if owner.
    let items: Vec<String> = kubetailor
        .client
        .get(format!("{}/list?owner={user}&filter=name", kubetailor.url))
        .send()
        .await
        .unwrap()
        .json::<Vec<String>>()
        .await
        .unwrap();

    if items.into_iter().any(|name| name == form.name) {
        kubetailor
            .client
            .post(format!(
                "{}/{}/restart?owner={user}",
                kubetailor.url, form.name
            ))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        Ok(see_other("/deployments"))
    } else {
        Ok(HttpResponse::NotFound().body(format!("Deployment {} not found", form.name)))
    }
}
