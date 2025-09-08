use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RunListItemVm {
    pub run_id: String,
    pub ritual_id: String,
    pub start_ts: String,
    pub status: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RunDetailVm {
    pub run_id: String,
    pub ritual_id: String,
    pub events: Vec<EventVm>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EventVm {
    pub ts: String,
    pub event: String,
    pub state_from: Option<String>,
    pub state_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

fn tojson_filter(value: &tera::Value, _: &HashMap<String, tera::Value>) -> tera::Result<tera::Value> {
    let s = serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".into());
    Ok(tera::Value::String(s))
}

pub fn register_filters(tera: &mut tera::Tera) {
    tera.register_filter("tojson", tojson_filter);
    // Back-compat alias used in templates
    let _ = tera.register_filter("json", tojson_filter);
}
