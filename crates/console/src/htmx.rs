use actix_web::HttpRequest;

pub trait HtmxRequest {
    fn is_htmx(&self) -> bool;
    fn htmx_trigger(&self) -> Option<&str>;
    fn htmx_target(&self) -> Option<&str>;
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
