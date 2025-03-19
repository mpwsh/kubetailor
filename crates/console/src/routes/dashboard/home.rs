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

    let data = json!({
        "title": "Home",
        "initial": !req.is_htmx(),
        "big_container": true,
        "user": user,
    });

    let body = hb.render("home", &data).unwrap();
    Ok(HttpResponse::Ok().body(body))
}
