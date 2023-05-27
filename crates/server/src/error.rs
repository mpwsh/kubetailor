use std::fmt;
#[derive(Debug)]
pub enum TappRequestError {
    Domain(String),
    Image(String),
    Name(String),
}

impl fmt::Display for TappRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TappRequestError::Domain(msg) => write!(f, "Invalid domain: {}", msg),
            TappRequestError::Name(msg) => write!(f, "Invalid tapp name: {}", msg),
            TappRequestError::Image(msg) => write!(f, "Invalid image: {}", msg),
        }
    }
}
