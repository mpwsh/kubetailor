use crate::routes::prelude::*;

#[derive(Deserialize)]
pub struct BasicForm {
    pub name: String,
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
        "head": format!("Delete {}?", params.name),
        "subtitle": "The deployment, its volumes and its network rules are removed. This cannot be undone.",
        "tapp_name": params.name,
        "user": user,
    });
    let body = hb.render("deployments/delete", &data).map_err(e500)?;
    Ok(HttpResponse::Ok().body(body))
}

/// Confirms the deletion and answers with the progress view, which then polls [`status`].
pub async fn form(
    hb: web::Data<Handlebars<'_>>,
    form: web::Form<BasicForm>,
    kubetailor: web::Data<Kubetailor>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();

    if !deployments::owns(&form.name, &user, &kubetailor).await? {
        return Ok(HttpResponse::NotFound().body(format!("Deployment {} not found", form.name)));
    }
    kubetailor
        .client
        .delete(format!("{}/{}?owner={user}", kubetailor.url, form.name))
        .send()
        .await
        .map_err(e500)?
        .error_for_status()
        .map_err(e500)?;

    if !req.is_htmx() {
        return Ok(see_other("/deployments"));
    }
    let data = json!({
        "initial": false,
        "title": "Destroying deployment",
        "tapp_name": form.name,
        "user": user,
    });
    let body = hb.render("deployments/destroying", &data).map_err(e500)?;
    Ok(HttpResponse::Ok().body(body))
}

/// Polled by the progress view: the same fragment while the tapp still exists, a client-side
/// redirect to the list once it is gone.
pub async fn status(
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

    if !deployments::owns(&params.name, &user, &kubetailor).await? {
        return Ok(redirect(&req, "/deployments"));
    }
    let body = hb
        .render(
            "deployments/destroying-status",
            &json!({ "tapp_name": params.name }),
        )
        .map_err(e500)?;
    Ok(HttpResponse::Ok().body(body))
}
