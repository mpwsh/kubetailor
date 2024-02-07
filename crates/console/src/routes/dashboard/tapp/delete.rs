use crate::{errors::ApiError, routes::prelude::*};

#[derive(Deserialize)]
pub struct BasicForm {
    pub name: String,
    pub loading: Option<bool>,
}

pub async fn page(
    hb: web::Data<Handlebars<'_>>,
    session: TypedSession,
    params: web::Query<BasicForm>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    let data = json!({
        "title": "Destroying tapp",
        "head": format!("Destroying Tapp: {}", params.name),
        "subtitle": "This action CANNOT be reverted. Proceed?",
        "tapp_name": params.name,
        "loading": params.loading.unwrap_or(false),
        "user": user,
    });
    let body = hb.render("forms/delete", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
}

pub async fn form(
    form: web::Form<BasicForm>,
    session: TypedSession,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, ApiError> {
    let user = if let Some(email) = session.get_user().map_err(e500).unwrap() {
        email
    } else {
        return Ok(see_other("/login"));
    };
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
            .delete(&format!("{}/{}?owner={user}", kubetailor.url, form.name))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        Ok(see_other(&format!(
            "/dashboard/delete?name={}&loading=true",
            form.name
        )))
    } else {
        Ok(HttpResponse::NotFound().body(format!("Tapp {} not found", form.name)))
    }
}
