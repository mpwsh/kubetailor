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

    let action = Action::new("Deploy").form().url("/dashboard/tapp/new");

    let data = json!({
        "title": "New Deployment",
        "return_url": "/dashboard",
        "action": action,
        "user": user,
    });

    let body = hb.render("new", &data).unwrap();

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
                    "/dashboard/tapp/deploying?name={}",
                    tapp.name
                )))
            } else {
                FlashMessage::info(response.text().await.unwrap()).send();
                Ok(see_other("/dashboard/error"))
            }
        },
        Err(e) => Ok(HttpResponse::BadRequest().body(e.to_string())),
    }
}
