use anyhow::{anyhow, Context, Result};
use jsonschema::{Draft, Validator};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize)]
pub struct Bundle {
    pub nats: Nats,
    #[serde(default)]
    pub stream: Stream,
    #[serde(default)]
    pub operate_ui: OperateUi,
    #[serde(default)]
    pub seed: Seed,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Nats {
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Stream {
    #[serde(default = "default_stream_name")]
    pub name: String,
    #[serde(default = "default_subjects")]
    pub subjects: Vec<String>,
    #[serde(rename = "duplicateWindowSeconds", default = "default_dup_window")]
    pub duplicate_window_seconds: u64,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct OperateUi {
    #[serde(rename = "baseUrl")]
    pub base_url: Option<String>,
    #[serde(rename = "approverAllowlist")]
    pub approver_allowlist: Option<Vec<String>>,
    #[serde(rename = "adminToken")]
    pub admin_token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Seed {
    pub enabled: Option<bool>,
    pub runs: Option<Vec<RunSpec>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RunSpec {
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "ritualId")]
    pub ritual_id: String,
    pub gates: Option<Vec<GateSpec>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GateSpec {
    #[serde(rename = "gateId")]
    pub gate_id: String,
    pub requester: String,
    #[serde(rename = "ttlSeconds")]
    pub ttl_seconds: Option<u64>,
}

fn default_stream_name() -> String {
    "RITUAL_EVENTS".into()
}
fn default_subjects() -> Vec<String> {
    vec!["demon.ritual.v1.>".into()]
}
fn default_dup_window() -> u64 {
    120
}

pub fn load_bundle(path: &Path) -> Result<Bundle> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("read bundle: {}", path.display()))?;
    let interpolated = interpolate_env(&raw);
    let bundle: Bundle = serde_yaml::from_str(&interpolated).context("parse bundle YAML")?;
    validate_against_schema(&interpolated)?;
    Ok(bundle)
}

fn validate_against_schema(yaml_text: &str) -> Result<()> {
    let schema_text = include_str!("../../../contracts/schemas/bootstrap.bundle.v0.json");
    let schema_json: JsonValue = serde_json::from_str(schema_text).context("parse schema JSON")?;
    let schema_owned: JsonValue = schema_json; // move ownership
                                               // Convert YAML to JSON for validation
    let doc_yaml: serde_yaml::Value = serde_yaml::from_str(yaml_text)?;
    let doc_json = serde_json::to_value(doc_yaml)?;
    // Leak the schema JSON to extend lifetime (acceptable for CLI process)
    let boxed = Box::new(schema_owned);
    let leaked: &'static JsonValue = Box::leak(boxed);
    let compiled = Validator::options()
        .with_draft(Draft::Draft7)
        .build(leaked)?;
    if let Err(err) = compiled.validate(&doc_json) {
        let mut msg = String::from("bundle schema validation errors:\n");
        msg.push_str(&format!("- {}\n", err));
        return Err(anyhow!(msg));
    }
    Ok(())
}

fn interpolate_env(s: &str) -> String {
    // Supports ${VAR} and ${VAR:-default}
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            if let Some(end) = s[i + 2..].find('}') {
                let token = &s[i + 2..i + 2 + end];
                let parts: Vec<&str> = token.splitn(2, ":-").collect();
                let key = parts[0];
                let default = if parts.len() == 2 {
                    Some(parts[1])
                } else {
                    None
                };
                let val = std::env::var(key)
                    .ok()
                    .or_else(|| default.map(|d| d.to_string()))
                    .unwrap_or_default();
                out.push_str(&val);
                i += 2 + end + 1; // skip ${...}
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

pub fn canonicalize_bundle_to_bytes(path: &Path) -> Result<Vec<u8>> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("read bundle: {}", path.display()))?;
    let interpolated = interpolate_env(&raw);
    let yaml: serde_yaml::Value = serde_yaml::from_str(&interpolated)?;
    let json_value = yaml_to_canonical_json(yaml);
    let bytes = serde_json::to_vec(&json_value)?;
    Ok(bytes)
}

fn yaml_to_canonical_json(v: serde_yaml::Value) -> serde_json::Value {
    match v {
        serde_yaml::Value::Null => serde_json::Value::Null,
        serde_yaml::Value::Bool(b) => serde_json::Value::Bool(b),
        serde_yaml::Value::Number(n) => serde_json::to_value(n).unwrap_or(serde_json::Value::Null),
        serde_yaml::Value::String(s) => serde_json::Value::String(s),
        serde_yaml::Value::Sequence(seq) => {
            let arr: Vec<serde_json::Value> = seq.into_iter().map(yaml_to_canonical_json).collect();
            serde_json::Value::Array(arr)
        }
        serde_yaml::Value::Mapping(map) => {
            let mut bt: BTreeMap<String, serde_json::Value> = BTreeMap::new();
            for (k, v) in map.into_iter() {
                let ks = match k {
                    serde_yaml::Value::String(s) => s,
                    other => serde_yaml::to_string(&other).unwrap_or_default(),
                };
                bt.insert(ks, yaml_to_canonical_json(v));
            }
            serde_json::to_value(bt).unwrap()
        }
        _ => serde_json::Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interpolate_basic_and_default() {
        std::env::set_var("FOO", "bar");
        let s = "x=${FOO},y=${MISSING:-def}";
        assert_eq!(interpolate_env(s), "x=bar,y=def");
    }
}
