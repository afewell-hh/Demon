use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RitualInvocationRequest {
    pub app: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub run_id: String,
    pub app: String,
    pub ritual: String,
    pub version: String,
    pub status: RunStatus,
    #[serde(with = "serde_rfc3339")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "serde_rfc3339")]
    pub updated_at: DateTime<Utc>,
    #[serde(default, with = "serde_rfc3339::option")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub parameters: serde_json::Value,
    #[serde(default)]
    pub result_envelope: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
}

impl RunRecord {
    pub fn summary(&self) -> RunSummary {
        RunSummary {
            run_id: self.run_id.clone(),
            app: self.app.clone(),
            ritual: self.ritual.clone(),
            version: self.version.clone(),
            status: self.status,
            created_at: self.created_at,
            updated_at: self.updated_at,
            completed_at: self.completed_at,
        }
    }

    pub fn detail(&self) -> RunDetail {
        RunDetail {
            run_id: self.run_id.clone(),
            app: self.app.clone(),
            ritual: self.ritual.clone(),
            version: self.version.clone(),
            status: self.status,
            created_at: self.created_at,
            updated_at: self.updated_at,
            completed_at: self.completed_at,
            parameters: self.parameters.clone(),
            result_envelope: self.result_envelope.clone(),
            error: self.error.clone(),
        }
    }
}

impl RunStatus {
    pub fn parse(input: &str) -> Option<Self> {
        match input.to_ascii_lowercase().as_str() {
            "pending" => Some(RunStatus::Pending),
            "running" => Some(RunStatus::Running),
            "completed" => Some(RunStatus::Completed),
            "failed" => Some(RunStatus::Failed),
            "canceled" => Some(RunStatus::Canceled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub run_id: String,
    pub app: String,
    pub ritual: String,
    pub version: String,
    pub status: RunStatus,
    #[serde(with = "serde_rfc3339")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "serde_rfc3339")]
    pub updated_at: DateTime<Utc>,
    #[serde(default, with = "serde_rfc3339::option")]
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDetail {
    pub run_id: String,
    pub app: String,
    pub ritual: String,
    pub version: String,
    pub status: RunStatus,
    #[serde(with = "serde_rfc3339")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "serde_rfc3339")]
    pub updated_at: DateTime<Utc>,
    #[serde(default, with = "serde_rfc3339::option")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub parameters: serde_json::Value,
    #[serde(default)]
    pub result_envelope: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCreatedResponse {
    pub run_id: String,
    pub status: RunStatus,
    pub created_at: String,
    pub links: RunLinks,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunLinks {
    pub run: String,
    pub envelope: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunListResponse {
    pub runs: Vec<RunSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEnvelopeResponse {
    pub run_id: String,
    pub envelope: serde_json::Value,
}

mod serde_rfc3339 {
    use chrono::{DateTime, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&dt.to_rfc3339())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        DateTime::parse_from_rfc3339(&s)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(serde::de::Error::custom)
    }

    pub mod option {
        use super::*;

        pub fn serialize<S>(value: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match value {
                Some(dt) => serializer.serialize_some(&dt.to_rfc3339()),
                None => serializer.serialize_none(),
            }
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let opt = Option::<String>::deserialize(deserializer)?;
            opt.map(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(serde::de::Error::custom)
            })
            .transpose()
        }
    }
}
