use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Logs {
    pub hits: Vec<LogBody>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LogBody {
    pub body: LogMessage,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LogMessage {
    pub message: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Query {
    pub aggs: Aggs,
    pub count_all: bool,
    pub end_timestamp: i64,
    pub format: Format,
    pub max_hits: i64,
    pub query: String,
    pub search_field: Vec<String>,
    pub snippet_fields: Vec<String>,
    pub sort_by: SortBy,
    pub start_offset: i64,
    pub start_timestamp: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aggs {}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortBy {
    pub sort_fields: Vec<SortField>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortField {
    pub field_name: String,
    pub sort_datetime_format: i64,
    pub sort_order: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Format {
    #[default]
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "pretty_json")]
    PrettyJson,
}

impl ToString for Format {
    fn to_string(&self) -> String {
        match self {
            Format::Json => "json".to_string(),
            Format::PrettyJson => "pretty_json".to_string(),
        }
    }
}

impl Query {
    pub fn new(query: &str) -> Query {
        Query {
            query: query.to_string(),
            ..Default::default()
        }
    }
}
