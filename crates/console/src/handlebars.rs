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
            table.contains(r#"hx-swap="outerHTML""#),
            "must replace itself, never nest"
        );
        assert!(table.contains("every 5s [document.getElementById('autorefresh')"));
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
        assert!(del.contains("hx-disabled-elt"));
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
        assert!(page.contains("every 30s [document.getElementById('log-autorefresh')"));
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
            "tapp": {"name": "", "domains": {"shared": "", "custom": null}, "container": {"image": "", "port": 80, "replicas": 1}, "git": null},
            "files": [], "custom_enabled": false,
        });
        let page = hb.render("deployments/editor", &data).unwrap();
        assert!(page.contains(r#"hx-post="/deployments/new""#));
        assert!(page.contains(r#"id="form-errors""#));
        assert!(!page.contains("post.js"));
        assert!(page.contains(r#"name="repository""#) && page.contains(r#"name="branch""#));
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
