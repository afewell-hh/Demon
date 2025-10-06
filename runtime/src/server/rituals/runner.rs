use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub run_id: String,
    pub ritual_id: String,
    pub capsule_ref: String,
    pub arguments: serde_json::Value,
}

#[async_trait]
pub trait RitualRunner: Send + Sync {
    async fn run(&self, plan: ExecutionPlan) -> anyhow::Result<serde_json::Value>;
}

#[derive(Debug, Default)]
pub struct EngineRitualRunner;

#[async_trait]
impl RitualRunner for EngineRitualRunner {
    async fn run(&self, plan: ExecutionPlan) -> anyhow::Result<serde_json::Value> {
        let router = crate::link::router::Router::new();
        let outputs = router
            .dispatch(
                &plan.capsule_ref,
                &plan.arguments,
                &plan.run_id,
                &plan.ritual_id,
            )
            .await?;

        Ok(serde_json::json!({
            "event": "ritual.completed:v1",
            "ritualId": plan.ritual_id,
            "runId": plan.run_id,
            "ts": chrono::Utc::now().to_rfc3339(),
            "outputs": outputs
        }))
    }
}
