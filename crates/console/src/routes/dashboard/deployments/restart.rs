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
        "title": "Restarting deployment",
        "head": format!("Restart {}?", params.name),
        "subtitle": "Pods are recreated one by one. Your application may be unavailable for a moment.",
        "tapp_name": params.name,
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
    if !deployments::owns(&form.name, &user, &kubetailor).await? {
        return Ok(HttpResponse::NotFound().body(format!("Deployment {} not found", form.name)));
    }
    kubetailor
        .client
        .post(format!(
            "{}/{}/restart?owner={user}",
            kubetailor.url, form.name
        ))
        .send()
        .await
        .map_err(e500)?
        .error_for_status()
        .map_err(e500)?;
    Ok(redirect(&req, "/deployments"))
}
