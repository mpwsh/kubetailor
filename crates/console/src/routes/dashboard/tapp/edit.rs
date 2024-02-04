use crate::routes::prelude::*;

#[derive(Deserialize)]
pub struct BasicForm {
    pub name: String,
    pub loading: Option<bool>,
}

pub async fn get(
    hb: web::Data<Handlebars<'_>>,
    session: TypedSession,
    params: web::Query<BasicForm>,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = if let Some(email) = session.get_user().map_err(e500)? {
        email
    } else {
        return Ok(see_other("/login"));
    };
    let mut tapp = tapp::get(&params.name, &user, kubetailor.clone()).await;
    log::info!("{}", serde_json::to_string(&tapp).unwrap());
    //todo replace when merging server code here
    tapp.domains.shared = tapp.domains.shared.replace(".kubetailor.io", "");
    //workaround for files index

    let mut print_files: Vec<(String, String, String)> = Vec::new();
    if let Some(files) = tapp.container.files.clone() {
        for (i, (key, value)) in files.into_iter().enumerate() {
            print_files.push((key.clone(), value.clone(), i.to_string()));
        }
    }

    let data = json!({
        "title": "Editing deployment",
        "head": format!("Editing {}", params.name),
        "is_form": true,
        "show_back": true,
        "custom_enabled": tapp.domains.custom.is_some(),
        "action": "Save",
        "tapp": tapp,
        "files": print_files,
        "user": user,
    });

    let body = hb.render("edit", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
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
    let old_tapp = tapp::get(&tapp.name, &user, kubetailor.clone()).await;
    if old_tapp.name.is_empty() {
        return Ok(HttpResponse::NotFound().body(format!("Tapp: {} -not found-", tapp.name)));
    }
    tapp.name = old_tapp.name.clone();
    tapp.owner = user;
    kubetailor
        .client
        .put(&kubetailor.url)
        .json(&tapp)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    Ok(see_other(&format!("/dashboard/loading?name={}", tapp.name)))
}
