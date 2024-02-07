use crate::routes::prelude::*;

#[derive(Deserialize)]
pub struct Form {
    pub name: String,
    pub query: Option<String>,
}

pub async fn get(
    hb: web::Data<Handlebars<'_>>,
    session: TypedSession,
    params: web::Query<Form>,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    let url = if let Some(query) = &params.query {
        format!(
            "{}/{}/logs?owner={}&query={}",
            kubetailor.url, params.name, user, query
        )
    } else {
        format!("{}/{}/logs?owner={}", kubetailor.url, params.name, user)
    };
    // Send the request
    let logs = kubetailor
        .client
        .get(url)
        .send()
        .await
        .expect("Failed to send request")
        .json::<Vec<String>>()
        .await
        .unwrap();
    let data = json!({
        "title": "Application logs",
        "head": format!("{} logs", params.name),
        "is_form": false,
        "show_back": true,
        "action": "Search",
        "logs": logs,
        "user": user,
    });

    let body = hb.render("logs", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
}
