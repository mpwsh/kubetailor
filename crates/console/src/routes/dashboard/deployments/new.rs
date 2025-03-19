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
        "action": action,
        "user": user,
    });

    let body = hb.render("deployments/editor", &data).unwrap();

    Ok(HttpResponse::Ok()
        .insert_header(ContentType(mime::TEXT_HTML))
        .body(body))
}

pub async fn form(
    mut tapp: web::Json<TappConfig>,
    kubetailor: web::Data<Kubetailor>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = req
        .extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string();

    tapp.owner = user;
    log::info!("{tapp:#?}");

    let res = kubetailor
        .client
        .post(&kubetailor.url)
        .json(&tapp)
        .send()
        .await;

    match res {
        Ok(response) => {
            if response.status().is_success() {
                Ok(see_other(&format!(
                    "/deployments/deploying?name={}",
                    tapp.name
                )))
            } else {
                FlashMessage::info(response.text().await.unwrap()).send();
                Ok(see_other("/error"))
            }
        },
        Err(e) => Ok(HttpResponse::BadRequest().body(e.to_string())),
    }
}
