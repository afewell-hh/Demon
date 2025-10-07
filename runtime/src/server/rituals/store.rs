use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::models::{RunRecord, RunStatus};

#[derive(Debug, Clone)]
pub struct RunStore {
    path: PathBuf,
    state: Arc<RwLock<RunDb>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RunDb {
    runs: HashMap<String, RunRecord>,
}

impl RunStore {
    pub fn open(path: PathBuf) -> Result<Self> {
        let state = if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("reading run store at {}", path.display()))?;
            serde_json::from_str(&raw).with_context(|| "parsing run store JSON")?
        } else {
            RunDb::default()
        };

        Ok(Self {
            path,
            state: Arc::new(RwLock::new(state)),
        })
    }

    pub async fn insert(&self, record: RunRecord) -> Result<RunRecord> {
        let mut guard = self.state.write().await;
        guard.runs.insert(record.run_id.clone(), record.clone());
        let snapshot = guard.clone();
        drop(guard);
        persist(&self.path, snapshot).await?;
        Ok(record)
    }

    pub async fn update<F>(&self, run_id: &str, update: F) -> Result<Option<RunRecord>>
    where
        F: FnOnce(&mut RunRecord),
    {
        let mut guard = self.state.write().await;
        if let Some(record) = guard.runs.get_mut(run_id) {
            update(record);
            let updated = record.clone();
            let snapshot = guard.clone();
            drop(guard);
            persist(&self.path, snapshot).await?;
            Ok(Some(updated))
        } else {
            Ok(None)
        }
    }

    pub async fn get(&self, run_id: &str) -> Option<RunRecord> {
        let guard = self.state.read().await;
        guard.runs.get(run_id).cloned()
    }

    pub async fn list_by_app_ritual(&self, app: &str, ritual: &str) -> Vec<RunRecord> {
        let guard = self.state.read().await;
        let mut runs: Vec<RunRecord> = guard
            .runs
            .values()
            .filter(|record| record.app == app && record.ritual == ritual)
            .cloned()
            .collect();

        runs.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.run_id.cmp(&a.run_id))
        });

        runs
    }

    pub async fn mark_failed(&self, run_id: &str, message: String) -> Result<()> {
        self.update(run_id, |record| {
            record.status = RunStatus::Failed;
            record.updated_at = chrono::Utc::now();
            record.completed_at = Some(record.updated_at);
            record.error = Some(message);
        })
        .await?
        .context("run not found")?;
        Ok(())
    }
}

async fn persist(path: &Path, db: RunDb) -> Result<()> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating run store directory {}", parent.display()))?;
        }
        let json = serde_json::to_vec_pretty(&db).context("serializing run store")?;
        std::fs::write(&path, json)
            .with_context(|| format!("writing run store to {}", path.display()))?;
        Ok::<(), anyhow::Error>(())
    })
    .await
    .context("joining run store persistence task")??;

    Ok(())
}
