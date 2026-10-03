use std::fmt;
#[derive(Debug)]
pub enum TappRequestError {
    Domain(String),
    Image(String),
    Name(String),
    Port(String),
    Region(String),
    Resources(String),
    Volume(String),
}

impl fmt::Display for TappRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TappRequestError::Domain(msg) => write!(f, "Invalid domain: {}", msg),
            TappRequestError::Name(msg) => write!(f, "Invalid tapp name: {}", msg),
            TappRequestError::Image(msg) => write!(f, "Invalid image: {}", msg),
            TappRequestError::Port(msg) => write!(f, "Invalid ports: {}", msg),
            TappRequestError::Region(msg) => write!(f, "Invalid region: {}", msg),
            TappRequestError::Resources(msg) => write!(f, "Invalid resources: {}", msg),
            TappRequestError::Volume(msg) => write!(f, "Invalid volumes: {}", msg),
        }
    }
}
