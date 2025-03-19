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
        "title": "Destroying deployment",
        "head": format!("Destroying deployment: {}", params.name),
        "subtitle": "This action CANNOT be reverted. Proceed?",
        "tapp_name": params.name,
        "loading": params.loading.unwrap_or(false),
        "user": user,
    });
    let body = hb.render("deployments/delete", &data).unwrap();

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
            .delete(format!("{}/{}?owner={user}", kubetailor.url, form.name))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        Ok(see_other(&format!(
            "/deployments/delete?name={}&loading=true",
            form.name
        )))
    } else {
        Ok(HttpResponse::NotFound().body(format!("Deployment {} not found", form.name)))
    }
}
