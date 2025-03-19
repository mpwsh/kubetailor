use crate::routes::prelude::*;

#[derive(Deserialize)]
pub struct BasicForm {
    pub name: String,
    pub loading: Option<bool>,
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

    let mut tapp = deployments::get(&params.name, &user, kubetailor.clone()).await;

    let action = Action::new("Save").form().url("/deployments/edit");

    //TODO: replace when merging server code here
    tapp.domains.shared = tapp.domains.shared.replace(".kubetailor.io", "");

    let mut print_files: Vec<(String, String, String)> = Vec::new();
    if let Some(files) = tapp.container.files.clone() {
        for (i, (key, value)) in files.into_iter().enumerate() {
            print_files.push((key.clone(), value.clone(), i.to_string()));
        }
    }

    let data = json!({
        "initial": !req.is_htmx(),
        "title": "Editing deployment",
        "head": format!("Editing {}", tapp.name),
        "custom_enabled": tapp.domains.custom.is_some(),
        "action": action,
        "tapp": tapp,
        "files": print_files,
        "user": user,
    });

    let body = hb.render("deployments/editor", &data).unwrap();

    Ok(HttpResponse::Ok().body(body))
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
    let old_tapp = deployments::get(&tapp.name, &user, kubetailor.clone()).await;
    if old_tapp.name.is_empty() {
        return Ok(HttpResponse::NotFound().body(format!("Deployment: {} not found", tapp.name)));
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
    Ok(see_other(&format!(
        "/deployments/deploying?name={}",
        tapp.name
    )))
}
