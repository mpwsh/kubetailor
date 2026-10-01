use crate::routes::prelude::*;

#[derive(Deserialize)]
pub struct BasicForm {
    pub name: String,
}

pub async fn page(
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

    let mut tapp = deployments::get(&params.name, &user, &kubetailor).await;
    if tapp.name.is_empty() {
        return Ok(HttpResponse::NotFound().body(""));
    }
    let config = kubetailor.config().await;

    let action = Action::new("Save").form().url("/deployments/edit");

    // The API stores the full hostname; the wizard edits the subdomain in front of the suffix.
    if let Some(domains) = &mut tapp.domains {
        if let Some(subdomain) = domains
            .shared
            .strip_suffix(&format!(".{}", config.base_domain))
            .filter(|_| !config.base_domain.is_empty())
        {
            domains.shared = subdomain.to_owned();
        }
    }

    let data = json!({
        "initial": !req.is_htmx(),
        "title": "Editing deployment",
        "head": format!("Editing {}", tapp.name),
        "return_url": format!("/deployments/view?name={}", tapp.name),
        "action": action,
        "port_rows": deployments::form::port_rows(Some(&tapp.container)),
        "max_ports": deployments::form::MAX_PORTS,
        "config": config,
        "tapp": tapp,
        "user": user,
    });

    let body = hb.render("deployments/editor", &data).map_err(e500)?;

    Ok(HttpResponse::Ok().body(body))
}
/// Saves the wizard's edits. The name cannot change (it is the resource identity), so the
/// existing one is kept regardless of what the form says.
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

    let config = kubetailor.config().await;
    let mut tapp = match deployments::form::tapp_from_form(&form, &config) {
        Ok(tapp) => tapp,
        Err(message) => return Ok(form_error(&message)),
    };
    let old_tapp = deployments::get(&tapp.name, &user, &kubetailor).await;
    if old_tapp.name.is_empty() {
        return Ok(form_error(&format!("Deployment {} not found", tapp.name)));
    }
    tapp.name = old_tapp.name.clone();
    tapp.owner = user;

    let response = kubetailor
        .client
        .put(&kubetailor.url)
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
