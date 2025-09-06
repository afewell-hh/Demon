use anyhow::Result;

/// Link-name router stub: resolves a functionRef to a capsule call.
/// Milestone 0 supports only the `echo` capsule with `{ message: String }`.
pub struct Router;

impl Router {
    pub fn new() -> Self { Self }

    /// Dispatch a functionRef by name with JSON arguments and return JSON output.
    pub fn dispatch(&self, ref_name: &str, args: &serde_json::Value) -> Result<serde_json::Value> {
        match ref_name {
            "echo" => {
                let msg = args.get("message").and_then(|v| v.as_str()).unwrap_or("");
                let out = capsules_echo::echo(msg.to_string());
                Ok(serde_json::json!({ "printed": out }))
            }
            other => anyhow::bail!("unknown functionRef: {other}"),
        }
    }
}
