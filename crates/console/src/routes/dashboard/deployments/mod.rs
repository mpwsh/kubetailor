pub mod delete;
pub mod edit;
pub mod form;
pub mod health;
pub mod new;
pub mod restart;
pub mod review;

use futures::future::join_all;

use crate::routes::prelude::*;

#[derive(Deserialize)]
pub struct BasicForm {
    pub name: String,
}

/// Element id of the self-refreshing deployments table; the handler renders only the table when
/// htmx says that is the target.
pub const TABLE_ID: &str = "deployments-table";
/// Element id of the self-polling deploy progress view.
pub const DEPLOY_STATUS_ID: &str = "deploy-status";

fn user_of(req: &HttpRequest) -> String {
    req.extensions()
        .get::<UserId>()
        .expect("UserId should be present after middleware check")
        .to_string()
}

/// Names of the deployments `owner` has.
pub async fn names(owner: &str, kubetailor: &Kubetailor) -> Result<Vec<String>, actix_web::Error> {
    kubetailor
        .client
        .get(format!(
            "{}/list?owner={}&filter=name",
            kubetailor.url, owner
        ))
        .send()
        .await
        .map_err(e500)?
        .error_for_status()
        .map_err(e500)?
        .json::<Vec<String>>()
        .await
        .map_err(e500)
}

/// Whether `owner` has a deployment called `name`. Every action on a deployment checks this
/// first, so nobody can act on someone else's by guessing a name.
pub async fn owns(
    name: &str,
    owner: &str,
    kubetailor: &Kubetailor,
) -> Result<bool, actix_web::Error> {
    Ok(names(owner, kubetailor).await?.iter().any(|n| n == name))
}

/// One row of the deployments table: the name plus its health, or `null` health when the
/// backend could not answer (the row then shows "unknown" instead of failing the page).
async fn row(name: String, owner: &str, kubetailor: &Kubetailor) -> Value {
    let health = health::get(name.clone(), owner.to_owned(), kubetailor)
        .await
        .map_err(|e| log::warn!("health for {name}: {e}"))
        .ok();
    let started = health
        .as_ref()
        .and_then(|h| h.pointer("/deployment/state/running/startedAt"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let domain = health
        .as_ref()
        .and_then(|h| h.pointer("/domains/domains/0"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    json!({
        "name": name,
        "health": health,
        "started": started,
        "domain": domain,
        "actions": [
            {"title": "View",    "icon": "fas fa-eye",                   "link": format!("/deployments/view?name={name}")},
            {"title": "Logs",    "icon": "fas fa-file-text",             "link": format!("/logs?name={name}")},
            {"title": "Edit",    "icon": "fas fa-edit",                  "link": format!("/deployments/edit?name={name}")},
            {"title": "Restart", "icon": "fa-solid fa-arrow-rotate-left", "link": format!("/deployments/restart?name={name}")},
            {"title": "Delete",  "icon": "fas fa-trash",                 "link": format!("/deployments/delete?name={name}")},
        ],
    })
}

/// The deployments list. Health is rendered server-side for every row (fetched concurrently),
/// so the table is one fragment that refreshes itself; when htmx asks for the table only
/// (`HX-Target: deployments-table`), that is all that is rendered.
pub async fn page(
    hb: web::Data<Handlebars<'_>>,
    kubetailor: web::Data<Kubetailor>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = user_of(&req);
    let items = names(&user, &kubetailor).await?;
    let rows = join_all(items.into_iter().map(|n| row(n, &user, &kubetailor))).await;

    let data = json!({
        "initial": !req.is_htmx(),
        "title": "Deployments",
        "deployments": rows,
        "action": Action::new("New").url("/deployments/new"),
        "user": user,
    });
    let template = if req.targets(TABLE_ID) {
        "deployments/table"
    } else {
        "deployments/main"
    };
    let body = hb.render(template, &data).map_err(e500)?;
    Ok(HttpResponse::Ok().body(body))
}

pub async fn get(tapp_name: &str, owner: &str, kubetailor: &Kubetailor) -> TappConfig {
    let exists = owns(tapp_name, owner, kubetailor).await.unwrap_or(false);
    if !exists {
        return TappConfig::default();
    }
    let fetched = async {
        kubetailor
            .client
            .get(format!("{}/{}?owner={}", kubetailor.url, tapp_name, owner))
            .send()
            .await?
            .error_for_status()?
            .json::<TappConfig>()
            .await
    }
    .await;
    match fetched {
        Ok(tapp) => tapp,
        Err(e) => {
            log::warn!("fetching {tapp_name}: {e}");
            TappConfig::default()
        }
    }
}

pub async fn view(
    hb: web::Data<Handlebars<'_>>,
    params: web::Query<BasicForm>,
    req: HttpRequest,
    kubetailor: web::Data<Kubetailor>,
) -> Result<HttpResponse, actix_web::Error> {
    let user = user_of(&req);
    let tapp = get(&params.name, &user, &kubetailor).await;
    if tapp.name.is_empty() {
        return Ok(HttpResponse::NotFound().body(""));
    }

    let action = Action::new("Edit").url(&format!("/deployments/edit?name={}", tapp.name));
    let data = json!({
        "initial": !req.is_htmx(),
        "title": format!("{} Details", params.name),
        "return_url": "/deployments",
        "action": action,
        "user": user,
        "tapp": tapp,
    });
    let body = hb.render("deployments/view", &data).map_err(e500)?;
    Ok(HttpResponse::Ok().body(body))
}

/// Progress view after a create or edit. The status fragment polls this URL targeting itself;
/// the server renders it without a trigger once everything is ready, which ends the polling.
pub async fn deploying(
    hb: web::Data<Handlebars<'_>>,
    params: web::Query<BasicForm>,
    kubetailor: web::Data<Kubetailor>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let user = user_of(&req);
    let status = health::get(params.name.clone(), user.clone(), &kubetailor)
        .await
        .map_err(|e| log::warn!("health for {}: {e}", params.name))
        .ok();

    let flag = |ptr: &str| {
        status
            .as_ref()
            .and_then(|s| s.pointer(ptr))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let has_domains = status
        .as_ref()
        .and_then(|s| s.pointer("/domains/domains"))
        .and_then(Value::as_array)
        .is_some_and(|d| !d.is_empty());
    let ready = flag("/deployment/ready");
    let done = ready && (!has_domains || (flag("/domains/dns") && flag("/domains/ssl")));

    let data = json!({
        "initial": !req.is_htmx(),
        "title": "Deploying",
        "head": format!("Deploying {}", params.name),
        "tapp_name": params.name,
        "status": status,
        "has_domains": has_domains,
        "done": done,
        "user": user,
    });
    let template = if req.targets(DEPLOY_STATUS_ID) {
        "deployments/status"
    } else {
        "deployments/deploying"
    };
    let body = hb.render(template, &data).map_err(e500)?;
    Ok(HttpResponse::Ok().body(body))
}
