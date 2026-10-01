use crate::routes::prelude::*;

pub async fn page(
    hb: web::Data<Handlebars<'_>>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();

    let action = Action::new("Deploy").form().url("/deployments/new");

    let data = json!({
        "initial": !req.is_htmx(),
        "title": "New Deployment",
        "return_url": "/deployments",
        "action": action,
        "user": user,
    });

    let body = hb.render("deployments/editor", &data).map_err(e500)?;

    Ok(HttpResponse::Ok()
        .insert_header(ContentType(mime::TEXT_HTML))
        .body(body))
}

/// Creates the deployment from the wizard's form post. Problems come back as an inline error
/// (retargeted into `#form-errors`); success navigates to the progress view.
pub async fn form(
    form: web::Form<Vec<(String, String)>>,
    kubetailor: web::Data<Kubetailor>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();

    let mut tapp = match deployments::form::tapp_from_form(&form) {
        Ok(tapp) => tapp,
        Err(message) => return Ok(form_error(&message)),
    };
    tapp.owner = user;
    log::info!("creating {}", tapp.name);

    let response = kubetailor
        .client
        .post(&kubetailor.url)
        .json(&tapp)
        .send()
        .await
        .map_err(e500)?;
    if response.status().is_success() {
        Ok(redirect(
            &req,
            &format!("/deployments/deploying?name={}", tapp.name),
        ))
    } else {
        let message = response.text().await.unwrap_or_default();
        Ok(form_error(&message))
    }
}
