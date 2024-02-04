use crate::routes::prelude::*;

pub async fn get(
    hb: web::Data<Handlebars<'_>>,
    session: TypedSession,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    let data = json!({
        "title": "Editing deployment",
        "head": "New Tapp",
        "is_form": true,
        "show_back": true,
        "action": "Deploy",
        "user": user,
    });
    let body = hb.render("new", &data).unwrap();

    Ok(HttpResponse::Ok()
        .insert_header(ContentType(mime::TEXT_HTML))
        .body(body))
}

pub async fn post(
    mut tapp: web::Json<TappConfig>,
    session: TypedSession,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
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
                Ok(see_other(&format!("/dashboard/loading?name={}", tapp.name)))
            } else {
                // Ok(HttpResponse::BadRequest().body(response.text().await.unwrap().to_string()))
                FlashMessage::info(format!(
                    "Internal server error. Unable to initialize session. Please try again\n{err}",
                    err = response.text().await.unwrap(),
                ))
                .send();
                Ok(see_other("/dashboard/new"))
            }
        },
        Err(e) => Ok(HttpResponse::BadRequest().body(e.to_string())),
    }
}
