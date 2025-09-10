use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Bundle {
    pub nats: Nats,
    #[serde(default)]
    pub stream: Stream,
    #[serde(default)]
    pub operate_ui: OperateUi,
    #[serde(default)]
    pub seed: Seed,
}

#[derive(Debug, Deserialize)]
pub struct Nats { pub url: String }

#[derive(Debug, Deserialize, Default)]
pub struct Stream {
    #[serde(default = "default_stream_name")] pub name: String,
    #[serde(default = "default_subjects")] pub subjects: Vec<String>,
    #[serde(default = "default_dup_window")] pub duplicate_window_seconds: u64,
}

#[derive(Debug, Deserialize, Default)]
pub struct OperateUi { pub base_url: Option<String>, pub approver_allowlist: Option<Vec<String>> }

#[derive(Debug, Deserialize, Default)]
pub struct Seed { pub enabled: Option<bool> }

fn default_stream_name() -> String { "RITUAL_EVENTS".into() }
fn default_subjects() -> Vec<String> { vec!["demon.ritual.v1.>".into()] }
fn default_dup_window() -> u64 { 120 }
