use anyhow::Result;
use handlebars::{
    Context, DirectorySourceOptions, Handlebars, Helper, HelperResult, Output, RenderContext,
};

pub fn create_handlebars() -> Result<Handlebars<'static>> {
    let mut handlebars = Handlebars::new();

    // Register templates directory
    handlebars
        .register_templates_directory("./web/templates", DirectorySourceOptions::default())?;

    // Register helpers
    register_helpers(&mut handlebars);

    Ok(handlebars)
}

fn register_helpers(handlebars: &mut Handlebars) {
    // Concat helper: joins every parameter; strings as-is, numbers and booleans rendered, null and
    // missing as nothing.
    handlebars.register_helper(
        "concat",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &Context,
             _: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let mut joined = String::new();
                for param in h.params() {
                    match param.value() {
                        serde_json::Value::String(s) => joined.push_str(s),
                        serde_json::Value::Null => {}
                        other => joined.push_str(&other.to_string()),
                    }
                }
                out.write(&joined)?;
                Ok(())
            },
        ),
    );

    // Add helper
    handlebars.register_helper(
        "add",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &Context,
             _: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let param0 = h.param(0).and_then(|v| v.value().as_i64()).unwrap_or(0);
                let param1 = h.param(1).and_then(|v| v.value().as_i64()).unwrap_or(0);
                out.write(&(param0 + param1).to_string())?;
                Ok(())
            },
        ),
    );

    // `eq` is deliberately not registered here: handlebars' built-in `eq` returns a boolean, which
    // is what `{{#if (eq a b)}}` needs. A helper that writes "true"/"false" as text is always
    // truthy inside `#if`/`#unless`.

    // `json`: the value as JSON, HTML-escaped so it can seed an Alpine `x-data="..."` attribute
    // with `{{{json value}}}` (the browser decodes the entities before Alpine evaluates it).
    // Missing or null renders as `[]`, which is what a list-valued state wants.
    handlebars.register_helper(
        "json",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &Context,
             _: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let value = match h.param(0).map(|v| v.value()) {
                    None | Some(serde_json::Value::Null) => "[]".to_owned(),
                    Some(v) => v.to_string(),
                };
                out.write(&handlebars::html_escape(&value))?;
                Ok(())
            },
        ),
    );

    // Default helper
    handlebars.register_helper(
        "default",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &Context,
             _: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let value = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
                let default = h.param(1).and_then(|v| v.value().as_str()).unwrap_or("");
                out.write(if value.is_empty() { default } else { value })?;
                Ok(())
            },
        ),
    );
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Attribute lists in the templates span lines; compare them with whitespace collapsed.
    fn squeeze(page: &str) -> String {
        page.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// The templates live at the repository root; tests run with the crate as cwd.
    fn registry() -> Handlebars<'static> {
        let mut hb = Handlebars::new();
        let dir = ["./web/templates", "../../web/templates"]
            .into_iter()
            .find(|d| std::path::Path::new(d).is_dir())
            .expect("templates directory");
        hb.register_templates_directory(dir, DirectorySourceOptions::default())
            .unwrap();
        register_helpers(&mut hb);
        hb
    }

    fn health(ready: bool) -> serde_json::Value {
        json!({
            "deployment": {"ready": ready, "restarts": 0,
                           "replicas": {"ready": if ready {1} else {0}, "desired": 1},
                           "state": {"running": {"startedAt": "2026-10-01T00:00:00Z"}}},
            "domains": {"domains": ["hello.example.com"], "dns": ready, "ssl": ready}
        })
    }

    #[test]
    fn deployments_table_is_a_self_replacing_fragment() {
        let hb = registry();
        let data = json!({
            "initial": false, "title": "Deployments", "user": "x@y",
            "action": {"name": "New", "url": "/deployments/new", "is_form": false},
            "deployments": [
                {"name": "hello", "health": health(true), "started": "t", "domain": "hello.example.com",
                 "actions": [{"title": "Edit", "icon": "fas fa-edit", "link": "/deployments/edit?name=hello"}]},
                {"name": "broken", "health": null, "started": null, "domain": null, "actions": []}
            ],
        });
        // handlebars escapes `=` in attribute values (`&#x3D;`); browsers decode it, tests undo it.
        let table = hb
            .render("deployments/table", &data)
            .unwrap()
            .replace("&#x3D;", "=");
        assert!(table.contains(r#"id="deployments-table""#));
        assert!(
            table.contains(r#"hx-swap="outerMorph""#),
            "must morph over itself, never nest"
        );
        // htmx 4: the filter rides on the event name, the interval follows.
        assert!(table.contains("every[document.getElementById('autorefresh')?.checked] 5s"));
        assert!(table.contains("status-dot-green") && table.contains("status-dot-neutral"));
        assert!(
            table.contains(r#"hx-get="/deployments/edit?name=hello""#),
            "static action links"
        );
        assert_eq!(table.matches(r#"id="deployments-table""#).count(), 1);

        let page = hb.render("deployments/main", &data).unwrap();
        assert!(page.contains(r#"id="autorefresh""#));
        assert!(page.contains("<title>Deployments</title>"));
        assert!(!page.contains("<html"), "fragment when not initial");
    }

    #[test]
    fn deploy_status_polls_until_done_then_stops() {
        let hb = registry();
        let mut data = json!({
            "initial": false, "title": "Deploying", "tapp_name": "hello", "user": "x@y",
            "status": health(false), "has_domains": true, "done": false,
        });
        let polling = hb.render("deployments/status", &data).unwrap();
        assert!(polling.contains(r#"hx-trigger="every 2s""#));
        assert!(polling.contains("Containers ready: 0/1"));
        assert!(
            !polling.contains(r#"id="content""#),
            "no duplicate #content inside the page"
        );

        data["status"] = health(true);
        data["done"] = json!(true);
        let finished = hb.render("deployments/status", &data).unwrap();
        assert!(
            !finished.contains("hx-trigger"),
            "polling must stop when done"
        );
        assert!(finished.contains("Go to deployments"));
    }

    #[test]
    fn confirm_modals_and_destroying_render() {
        let hb = registry();
        let data = json!({
            "initial": false, "title": "t", "head": "Delete hello?", "subtitle": "Sure?",
            "tapp_name": "hello", "user": "x@y",
        });
        let del = hb.render("deployments/delete", &data).unwrap();
        assert!(del.contains(r#"hx-post="/deployments/delete""#));
        assert!(del.contains(r#"hx-vals='{"name": "hello"}'"#));
        assert!(del.contains(r#"hx-disable="this""#));
        assert!(!del.contains("hx-follow"));
        let restart = hb.render("deployments/restart", &data).unwrap();
        assert!(restart.contains(r#"hx-post="/deployments/restart""#));
        let destroying = hb.render("deployments/destroying", &data).unwrap();
        assert!(destroying.contains("/deployments/delete/status?name=hello"));
    }

    #[test]
    fn logs_view_renders_selection_and_interval() {
        let hb = registry();
        let data = json!({
            "initial": true, "title": "Application logs", "user": "x@y",
            "deployments": ["a", "b"], "name": "b", "query": "err", "interval": 30,
            "intervals": [{"seconds": 10, "label": "10s", "selected": false},
                          {"seconds": 30, "label": "30s", "selected": true}],
            "logs": ["line one", "line <two>"],
        });
        let page = hb.render("logs", &data).unwrap();
        assert!(page.contains("<html"));
        assert!(
            page.contains(r#"<option value="b" selected>"#),
            "built-in eq selects the deployment"
        );
        assert!(page.contains(r#"<option value="30" selected>"#));
        assert!(page.contains("every[document.getElementById('log-autorefresh')?.checked] 30s"));
        assert!(page.contains("line &lt;two&gt;"), "log lines are escaped");
        let pane = hb
            .render("logs/pane", &json!({"name": null, "logs": []}))
            .unwrap();
        assert!(
            !pane.contains("hx-trigger"),
            "nothing to poll without a deployment"
        );
    }

    #[test]
    fn editor_posts_a_plain_form() {
        let hb = registry();
        let data = json!({
            "initial": false, "title": "New Deployment", "user": "x@y", "return_url": "/deployments",
            "action": {"name": "Deploy", "url": "/deployments/new", "is_form": true},
            "config": {"baseDomain": "apps.example.com", "nodePortRange": [1024, 29999]},
            "port_rows": [{"port": "80", "protocol": "HTTP", "expose": "cluster"}],
            "max_ports": 5,
        });
        let page = hb.render("deployments/editor", &data).unwrap();
        assert!(page.contains(r#"hx-post="/deployments/new""#));
        assert!(page.contains(r#"hx-status:422="target:#form-errors swap:innerHTML""#));
        assert!(
            page.contains(r#"hx-disable="[form=editForm]""#),
            "the submit button is outside the form"
        );
        assert!(page.contains(r#"id="form-errors""#));
        assert!(!page.contains("post.js") && !page.contains("key_value.js"));
        assert!(page.contains(r#"name="repository""#) && page.contains(r#"name="branch""#));
        // No HTTP port field in Source: ports are the Network step's, seeded with HTTP on 80.
        assert!(!page.contains(r#"name="port""#));
        assert!(page.contains(r#"x-data="{ ports: [{&quot;expose&quot;:&quot;cluster&quot;,&quot;port&quot;:&quot;80&quot;,&quot;protocol&quot;:&quot;HTTP&quot;}], max: 5 }""#));
        assert!(page.contains(r#"name="port_number""#) && page.contains(r#"name="port_expose""#));
        // Basic has the region; Network has the group; Domains shows the real suffix.
        assert!(page.contains(r#"name="region""#) && page.contains(r#"name="group""#));
        assert!(page.contains(".apps.example.com"));
        assert!(!page.contains("kubetailor.io"));
        // Repeaters seed from nothing as empty lists.
        assert_eq!(
            page.matches("Object.entries([])").count(),
            4,
            "volumes, files, env, secrets"
        );
    }

    #[test]
    fn editor_seeds_an_existing_app() {
        let hb = registry();
        let data = json!({
            "initial": false, "title": "Editing deployment", "user": "x@y", "return_url": "/deployments",
            "action": {"name": "Save", "url": "/deployments/edit", "is_form": true},
            "config": {"baseDomain": "apps.example.com", "nodePortRange": [1024, 29999]},
            "port_rows": [{"port": "7777", "protocol": "UDP", "expose": "node"}],
            "max_ports": 5,
            "tapp": {
                "name": "game", "region": "scl", "group": "games", "domains": null,
                "container": {"image": "game:1", "port": null, "replicas": 1,
                              "ports": [{"port": 7777, "protocol": "UDP", "expose": "node"}],
                              "volumes": {"/data": "1Gi"}, "files": {"/etc/game.toml": "x = 1\n"}},
                "env": {"A": "1"}, "secrets": null, "git": null,
            },
        });
        let page = squeeze(&hb.render("deployments/editor", &data).unwrap());
        assert!(page.contains(r#"name="region" placeholder="any" autocomplete="off" value="scl""#));
        assert!(page
            .contains(r#"name="group" placeholder="optional" autocomplete="off" value="games""#));
        assert!(page.contains(r#"ports: [{&quot;expose&quot;:&quot;node&quot;,&quot;port&quot;:&quot;7777&quot;,&quot;protocol&quot;:&quot;UDP&quot;}]"#));
        assert!(page.contains(r#"Object.entries({&quot;/data&quot;:&quot;1Gi&quot;})"#));
        // handlebars escapes `=` as well; the browser decodes it before Alpine sees the JSON.
        assert!(page
            .contains(r#"Object.entries({&quot;/etc/game.toml&quot;:&quot;x &#x3D; 1\n&quot;})"#));
        assert!(page.contains(r#"Object.entries({&quot;A&quot;:&quot;1&quot;})"#));
        // No domains: the subdomain input is empty, not "null".
        assert!(page.contains(r#"x-data="{ shared: '' }""#));
    }

    #[test]
    fn review_shows_summary_manifest_and_the_deploy_button() {
        let hb = registry();
        let data = json!({
            "tapp": {
                "name": "game", "region": "scl", "group": "games", "domains": {"shared": "game", "custom": null},
                "container": {"image": "game:1", "port": 8080, "replicas": 1, "ports": []},
                "env": null, "secrets": null, "git": null,
            },
            "base_domain": "apps.example.com",
            "manifest": "apiVersion: kubetailor.io/v1\nkind: TailoredApp\n",
            "action_label": "Deploy",
            "user": "x@y",
        });
        let page = hb.render("deployments/review", &data).unwrap();
        assert!(
            page.contains("game.apps.example.com"),
            "subdomain gets the suffix in review"
        );
        assert!(page.contains("kind: TailoredApp"));
        assert!(page.contains(r#"<button type="submit" form="editForm""#));
        assert!(page.contains("Deploy") && !page.contains("Review &amp; Deploy"));
        // Without a manifest the review still renders, with a note.
        let mut data = data;
        data["manifest"] = json!(null);
        let page = hb.render("deployments/review", &data).unwrap();
        assert!(page.contains("did not answer the manifest preview"));
    }

    #[test]
    fn editor_has_review_mode_and_no_top_bar_submit() {
        let hb = registry();
        let data = json!({
            "initial": false, "title": "New Deployment", "user": "x@y", "return_url": "/deployments",
            "action": {"name": "Deploy", "url": "/deployments/new", "is_form": true},
            "config": {"baseDomain": "apps.example.com", "nodePortRange": [1024, 29999]},
            "port_rows": [], "max_ports": 5,
        });
        let page = hb.render("deployments/editor", &data).unwrap();
        assert!(page.contains(r#"hx-post="/deployments/review""#));
        assert!(page.contains("Review &amp; Deploy"));
        assert!(page.contains(r#"id="review" x-show="review""#));
        assert!(page.contains("@review-ready.window"));
        // The only submit button is the review step's, which is not in this render.
        assert!(
            !page.contains(r#"type="submit""#),
            "top bar must not carry the submit"
        );
    }

    #[test]
    fn view_renders_every_section() {
        let hb = registry();
        let data = json!({
            "initial": false, "title": "game Details", "user": "x@y", "return_url": "/deployments",
            "action": {"name": "Edit", "url": "/deployments/edit?name=game", "is_form": false},
            "tapp": {
                "name": "game", "region": "scl", "group": "", "domains": null,
                "container": {"image": "game:1", "port": 8080, "replicas": 1,
                              "ports": [{"port": 7777, "protocol": "UDP", "expose": "node"}]},
                "env": null, "secrets": {"TOKEN": "s3cret"}, "git": null,
            },
        });
        let page = hb.render("deployments/view", &data).unwrap();
        assert!(page.contains("8080") && page.contains("public, same port"));
        assert!(page.contains("reached by IP"));
        assert!(
            page.contains("TOKEN") && !page.contains("s3cret"),
            "secret values stay hidden"
        );
    }

    #[test]
    fn json_helper_escapes_for_attributes() {
        let hb = registry();
        let out = hb
            .render_template(
                r#"<div x-data="{ ports: {{{json tapp.container.ports}}} }">"#,
                &json!({"tapp": {"container": {"ports": [{"port": 7777, "protocol": "UDP", "expose": "node"}]}}}),
            )
            .unwrap();
        assert_eq!(
            out,
            r#"<div x-data="{ ports: [{&quot;expose&quot;:&quot;node&quot;,&quot;port&quot;:7777,&quot;protocol&quot;:&quot;UDP&quot;}] }">"#
        );
        let out = hb
            .render_template("{{{json missing}}}", &json!({}))
            .unwrap();
        assert_eq!(out, "[]");
    }

    #[test]
    fn concat_renders_numbers() {
        let hb = registry();
        let out = hb
            .render_template("{{concat \"a\" 1 \"/\" b}}", &json!({"b": 2}))
            .unwrap();
        assert_eq!(out, "a1/2");
    }
}
