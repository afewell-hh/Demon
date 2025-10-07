use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use serde_json::Value as JsonValue;
use tracing::{error, info, warn};
use uuid::Uuid;

use super::models::{
    RitualInvocationRequest, RunCreatedResponse, RunDetail, RunLinks, RunListResponse, RunRecord,
    RunStatus,
};
use super::registry::{AppPackRegistry, CapsuleEntry, ResolvedInvocation};
use super::runner::{EngineRitualRunner, ExecutionPlan, RitualRunner};
use super::store::RunStore;

#[derive(Clone)]
pub struct RitualService {
    registry: AppPackRegistry,
    store: RunStore,
    runner: Arc<dyn RitualRunner>,
}

impl RitualService {
    pub fn new() -> Result<Self> {
        Self::with_runner(Arc::new(EngineRitualRunner))
    }

    pub fn with_runner(runner: Arc<dyn RitualRunner>) -> Result<Self> {
        let registry = AppPackRegistry::new()?;
        let store_path = default_store_path()?.join("runs.json");
        let store = RunStore::open(store_path)?;
        Ok(Self {
            registry,
            store,
            runner,
        })
    }

    pub fn with_dependencies(
        registry: AppPackRegistry,
        store: RunStore,
        runner: Arc<dyn RitualRunner>,
    ) -> Self {
        Self {
            registry,
            store,
            runner,
        }
    }

    pub async fn schedule_run(
        &self,
        ritual_name: &str,
        request: RitualInvocationRequest,
    ) -> Result<(RunRecord, RunCreatedResponse)> {
        let resolved = self
            .registry
            .resolve_invocation(ritual_name, &request)
            .context("resolving ritual invocation")?;

        let version = request
            .version
            .clone()
            .unwrap_or_else(|| resolved.manifest.metadata.version.clone());

        let now = Utc::now();
        let run_id = Uuid::new_v4().to_string();

        let plan = build_execution_plan(&resolved, &request.parameters, &run_id)?;

        let record = RunRecord {
            run_id: run_id.clone(),
            app: request.app.clone(),
            ritual: ritual_name.to_string(),
            version: version.clone(),
            status: RunStatus::Running,
            created_at: now,
            updated_at: now,
            completed_at: None,
            parameters: request.parameters.clone(),
            result_envelope: None,
            error: None,
        };

        self.store
            .insert(record.clone())
            .await
            .context("persisting run metadata")?;

        self.spawn_execution(plan, record.app.clone())?;

        let response = RunCreatedResponse {
            run_id: run_id.clone(),
            status: RunStatus::Running,
            created_at: now.to_rfc3339(),
            links: RunLinks {
                run: format!(
                    "/api/v1/rituals/{}/runs/{}?app={}",
                    ritual_name, run_id, record.app
                ),
                envelope: format!(
                    "/api/v1/rituals/{}/runs/{}/envelope?app={}",
                    ritual_name, run_id, record.app
                ),
            },
        };

        Ok((record, response))
    }

    pub async fn list_runs(
        &self,
        app: &str,
        ritual: &str,
        limit: Option<usize>,
        status: Option<RunStatus>,
    ) -> Result<RunListResponse> {
        let mut runs = self.store.list_by_app_ritual(app, ritual).await;
        if let Some(status_filter) = status {
            runs.retain(|run| run.status == status_filter);
        }
        if let Some(limit) = limit {
            runs.truncate(limit.min(500));
        }

        let summaries = runs.into_iter().map(|r| r.summary()).collect();
        Ok(RunListResponse {
            runs: summaries,
            next_page_token: None,
        })
    }

    pub async fn get_run(
        &self,
        app: &str,
        ritual: &str,
        run_id: &str,
    ) -> Result<Option<RunDetail>> {
        let maybe_run = self.store.get(run_id).await;
        if let Some(record) = maybe_run {
            if record.app == app && record.ritual == ritual {
                return Ok(Some(record.detail()));
            }
        }
        Ok(None)
    }

    pub async fn get_envelope(
        &self,
        app: &str,
        ritual: &str,
        run_id: &str,
    ) -> Result<Option<JsonValue>> {
        let maybe_run = self.store.get(run_id).await;
        if let Some(record) = maybe_run {
            if record.app == app && record.ritual == ritual {
                return Ok(record.result_envelope.clone());
            }
        }
        Ok(None)
    }

    fn spawn_execution(&self, plan: ExecutionPlan, app: String) -> Result<()> {
        let store = self.store.clone();
        let runner = Arc::clone(&self.runner);
        let run_id = plan.run_id.clone();
        let ritual_id = plan.ritual_id.clone();
        tokio::spawn(async move {
            info!(run = %run_id, ritual = %ritual_id, "starting ritual execution task");
            match runner.run(plan).await {
                Ok(envelope) => {
                    let now = Utc::now();
                    if let Err(err) = store
                        .update(&run_id, |record| {
                            record.status = RunStatus::Completed;
                            record.updated_at = now;
                            record.completed_at = Some(now);
                            record.result_envelope = Some(envelope.clone());
                        })
                        .await
                    {
                        error!(run = %run_id, %app, error = %err, "failed to persist completion metadata");
                    }
                }
                Err(err) => {
                    warn!(run = %run_id, %app, error = %err, "ritual execution failed");
                    let message = err.to_string();
                    if let Err(err) = store
                        .update(&run_id, |record| {
                            let now = Utc::now();
                            record.status = RunStatus::Failed;
                            record.updated_at = now;
                            record.completed_at = Some(now);
                            record.error = Some(message.clone());
                        })
                        .await
                    {
                        error!(run = %run_id, %app, error = %err, "failed to persist failure metadata");
                    }
                }
            }
        });

        Ok(())
    }
}

fn build_execution_plan(
    resolved: &ResolvedInvocation,
    parameters: &JsonValue,
    run_id: &str,
) -> Result<ExecutionPlan> {
    let step = resolved
        .ritual
        .steps
        .first()
        .ok_or_else(|| anyhow!("ritual must contain at least one step"))?;
    let capsule = resolved
        .manifest
        .capsules
        .iter()
        .find(|entry| match entry {
            CapsuleEntry::ContainerExec { name, .. } => name == &step.capsule,
            CapsuleEntry::Unsupported => false,
        })
        .ok_or_else(|| {
            anyhow!(
                "capsule '{}' referenced by ritual '{}' not found",
                step.capsule,
                resolved.ritual.name
            )
        })?;

    if !step.with.is_null() && !step.with.is_object() {
        return Err(anyhow!("Ritual step overrides must be an object"));
    }
    if !parameters.is_null() && !parameters.is_object() {
        return Err(anyhow!("Invocation parameters must be a JSON object"));
    }

    let (ref_name, args) = match capsule {
        CapsuleEntry::ContainerExec {
            name,
            image_digest,
            command,
            env,
            working_dir,
            outputs,
        } => {
            let mut base = serde_json::json!({
                "imageDigest": image_digest,
                "command": command,
                "env": env,
                "outputs": { "envelopePath": outputs.envelope_path },
            });

            if let Some(dir) = working_dir {
                if let Some(obj) = base.as_object_mut() {
                    obj.insert("workingDir".into(), JsonValue::String(dir.clone()));
                }
            }

            merge_json(&mut base, &step.with)?;
            merge_json(&mut base, parameters)?;

            if let Some(obj) = base.as_object_mut() {
                obj.insert("capsuleName".into(), JsonValue::String(name.clone()));
            }

            ("container-exec".to_string(), base)
        }
        CapsuleEntry::Unsupported => {
            return Err(anyhow!(
                "capsule '{}' uses an unsupported type for HTTP invocation",
                step.capsule
            ));
        }
    };

    Ok(ExecutionPlan {
        run_id: run_id.to_string(),
        ritual_id: format!(
            "{}::{}",
            resolved.manifest.metadata.name, resolved.ritual.name
        ),
        capsule_ref: ref_name,
        arguments: args,
    })
}

fn merge_json(target: &mut JsonValue, other: &JsonValue) -> Result<()> {
    if other.is_null() {
        return Ok(());
    }

    if let JsonValue::Object(target_map) = target {
        if let JsonValue::Object(other_map) = other {
            for (key, value) in other_map {
                merge_json(
                    target_map.entry(key.clone()).or_insert(JsonValue::Null),
                    value,
                )?;
            }
            return Ok(());
        }
    }

    *target = other.clone();
    Ok(())
}

fn default_store_path() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("DEMON_RUNTIME_HOME") {
        return Ok(PathBuf::from(dir));
    }
    if let Ok(dir) = std::env::var("DEMON_HOME") {
        return Ok(PathBuf::from(dir).join("runtime"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Ok(PathBuf::from(home).join(".demon").join("runtime"));
    }
    Err(anyhow!(
        "Unable to determine runtime store path. Set DEMON_RUNTIME_HOME or HOME"
    ))
}
