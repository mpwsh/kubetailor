use actix_web::HttpRequest;

/// The request headers htmx sends (`HX-Request`, `HX-Trigger`, `HX-Target`), as the server-side
/// signal for "render a fragment" versus "render the whole page".
pub trait HtmxRequest {
    fn is_htmx(&self) -> bool;
    fn htmx_trigger(&self) -> Option<&str>;
    fn htmx_target(&self) -> Option<&str>;
    /// True when the swap target is the element with this id, i.e. only that part of the page
    /// needs rendering (a polling table, a log pane).
    fn targets(&self, id: &str) -> bool {
        self.htmx_target() == Some(id)
    }
}

impl HtmxRequest for HttpRequest {
    fn is_htmx(&self) -> bool {
        self.headers().contains_key("hx-request")
    }

    fn htmx_trigger(&self) -> Option<&str> {
        self.headers()
            .get("hx-trigger")
            .and_then(|v| v.to_str().ok())
    }

    fn htmx_target(&self) -> Option<&str> {
        self.headers()
            .get("hx-target")
            .and_then(|v| v.to_str().ok())
    }
}
