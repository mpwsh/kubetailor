//! The wizard's last step: everything the person entered, plus the manifest the API would
//! create from it, before the deploy button. Posting the form here validates it exactly like
//! a deploy would, so a problem shows up in `#form-errors` (422) instead of after the fact.

use crate::routes::prelude::*;

/// Element the review fragment is swapped into; the editor shows it instead of the wizard once
/// the `review-ready` event (sent with the fragment) flips its mode.
pub const REVIEW_ID: &str = "review";

pub async fn form(
    hb: web::Data<Handlebars<'_>>,
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
    tapp.owner = user.clone();

    let manifest = match kubetailor.preview(&tapp).await {
        Ok(Ok(yaml)) => Some(yaml),
        Ok(Err(message)) => return Ok(form_error(&message)),
        Err(e) => {
            log::warn!("manifest preview unavailable: {e}");
            None
        }
    };

    // The wizard says what submitting means ("Deploy" for a new app, "Save" for an edit).
    let action_label = form
        .iter()
        .find(|(k, _)| k == "intent")
        .map(|(_, v)| v.trim())
        .filter(|v| !v.is_empty())
        .unwrap_or("Deploy");
    let region_name = tapp.region.as_deref().map(|r| config.region_name(r));
    let region_empty = tapp
        .region
        .as_deref()
        .is_some_and(|r| config.region_is_empty(r));
    let data = json!({
        "tapp": tapp,
        "base_domain": config.base_domain,
        "region_name": region_name,
        "region_empty": region_empty,
        "manifest": manifest,
        "action_label": action_label,
        "user": user,
    });
    let body = hb.render("deployments/review", &data).map_err(e500)?;
    Ok(HttpResponse::Ok()
        .insert_header(("HX-Trigger", "review-ready"))
        .body(body))
}
