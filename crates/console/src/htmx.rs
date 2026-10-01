use actix_web::HttpRequest;

/// The request headers htmx 4 sends, as the server-side signal for "render a fragment" versus
/// "render the whole page".
///
/// - `HX-Request: true` on every htmx request.
/// - `HX-Request-Type: partial | full` — `full` when the swap targets `<body>` or uses a
///   selection, which is what a back/forward restore does: htmx re-fetches the URL and picks
///   `[hx-history-elt]` out of the response, so that response has to be the whole page.
/// - `HX-Target: tag#id` of the swap target (htmx 2 sent the bare id).
/// - `HX-Source: tag#id` of the element that made the request (htmx 2: `HX-Trigger`).
pub trait HtmxRequest {
    /// An htmx request that wants a fragment for `#content` (or something inside it).
    fn is_htmx(&self) -> bool;
    fn htmx_target(&self) -> Option<&str>;
    /// True when the swap target is the element with this id, i.e. only that part of the page
    /// needs rendering (a polling table, a log pane).
    fn targets(&self, id: &str) -> bool {
        self.htmx_target().is_some_and(|t| element_id(t) == id)
    }
}

/// The id part of htmx 4's `tag#id` identifiers; a bare id (htmx 2, or an element without a
/// tag prefix) is returned as is.
fn element_id(identifier: &str) -> &str {
    identifier.split_once('#').map_or(identifier, |(_, id)| id)
}

impl HtmxRequest for HttpRequest {
    fn is_htmx(&self) -> bool {
        let header = |name: &str| self.headers().get(name).and_then(|v| v.to_str().ok());
        header("hx-request") == Some("true")
            && header("hx-request-type") != Some("full")
            && header("hx-history-restore-request") != Some("true")
    }

    fn htmx_target(&self) -> Option<&str> {
        self.headers()
            .get("hx-target")
            .and_then(|v| v.to_str().ok())
    }
}

#[cfg(test)]
mod tests {
    use actix_web::test::TestRequest;

    use super::*;

    #[test]
    fn target_accepts_both_spellings() {
        let req = TestRequest::default()
            .insert_header(("HX-Request", "true"))
            .insert_header(("HX-Target", "div#deployments-table"))
            .to_http_request();
        assert!(req.targets("deployments-table"));
        assert!(!req.targets("content"));
        let req = TestRequest::default()
            .insert_header(("HX-Target", "deployments-table"))
            .to_http_request();
        assert!(req.targets("deployments-table"));
    }

    #[test]
    fn history_restores_get_the_whole_page() {
        let partial = TestRequest::default()
            .insert_header(("HX-Request", "true"))
            .insert_header(("HX-Request-Type", "partial"))
            .to_http_request();
        assert!(partial.is_htmx());
        let restore = TestRequest::default()
            .insert_header(("HX-Request", "true"))
            .insert_header(("HX-Request-Type", "full"))
            .insert_header(("HX-History-Restore-Request", "true"))
            .to_http_request();
        assert!(!restore.is_htmx());
        assert!(!TestRequest::default().to_http_request().is_htmx());
    }
}
