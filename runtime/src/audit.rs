use metrics::{counter, gauge, histogram};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

/// Structured audit events for contract bundle lifecycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleAuditEvent {
    pub event_type: BundleEventType,
    pub tag: String,
    pub sha256: Option<String>,
    pub source: BundleSource,
    pub error: Option<String>,
    pub remediation_hint: Option<String>,
    pub metadata: BundleEventMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BundleEventType {
    /// Bundle successfully loaded from cache or download
    Loaded,
    /// Bundle verification failed (SHA-256 mismatch)
    VerificationFailed,
    /// Bundle fallback activated due to load failure
    FallbackActivated,
    /// Bundle download/refresh attempt started
    RefreshAttempt,
    /// Bundle download failed
    DownloadFailed,
    /// Bundle detected as stale
    StaleDetected,
    /// Bundle update/newer version detected
    UpdateDetected,
    /// Bundle status check performed
    StatusCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BundleSource {
    Cache,
    Download,
    Fallback,
    EmbeddedSchemas,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleEventMetadata {
    pub timestamp: String,
    pub duration_ms: Option<u64>,
    pub git_sha: Option<String>,
    pub bundle_timestamp: Option<String>,
    pub cache_path: Option<String>,
    pub network_error: Option<String>,
}

/// Bundle metrics recorder for Prometheus/monitoring
pub struct BundleMetrics;

impl BundleMetrics {
    /// Record a bundle state gauge (current status)
    pub fn record_bundle_status(status: &str) {
        gauge!("demon_bundle_status", 1.0, "state" => status.to_string());
    }

    /// Increment bundle operation counter
    pub fn increment_bundle_operation(operation: &str, result: &str) {
        counter!("demon_bundle_operations_total", 1,
            "operation" => operation.to_string(),
            "result" => result.to_string()
        );
    }

    /// Record bundle refresh latency
    pub fn record_refresh_duration(duration_ms: u64) {
        histogram!("demon_bundle_refresh_duration_ms", duration_ms as f64);
    }

    /// Record bundle age (for staleness monitoring)
    pub fn record_bundle_age_hours(age_hours: i64) {
        gauge!("demon_bundle_age_hours", age_hours as f64);
    }

    /// Record bundle verification failures
    pub fn increment_verification_failure(error_type: &str) {
        counter!("demon_bundle_verification_failures_total", 1,
            "error_type" => error_type.to_string()
        );
    }

    /// Record bundle fallback usage
    pub fn record_fallback_active(active: bool) {
        gauge!(
            "demon_bundle_fallback_active",
            if active { 1.0 } else { 0.0 }
        );
    }
}

/// Audit event recorder for structured logging and telemetry
pub struct BundleAuditor;

impl BundleAuditor {
    /// Emit a structured audit event for bundle operations
    pub fn emit_event(event: BundleAuditEvent) {
        let event_name = match event.event_type {
            BundleEventType::Loaded => "bundle.loaded",
            BundleEventType::VerificationFailed => "bundle.verification_failed",
            BundleEventType::FallbackActivated => "bundle.fallback_activated",
            BundleEventType::RefreshAttempt => "bundle.refresh_attempt",
            BundleEventType::DownloadFailed => "bundle.download_failed",
            BundleEventType::StaleDetected => "bundle.stale_detected",
            BundleEventType::UpdateDetected => "bundle.update_detected",
            BundleEventType::StatusCheck => "bundle.status_check",
        };

        // Emit structured tracing event
        match &event.event_type {
            BundleEventType::Loaded => {
                info!(
                    event = event_name,
                    tag = %event.tag,
                    source = ?event.source,
                    sha256 = %event.sha256.as_deref().unwrap_or("unknown"),
                    git_sha = %event.metadata.git_sha.as_deref().unwrap_or("unknown"),
                    duration_ms = event.metadata.duration_ms,
                    "Bundle loaded successfully"
                );
                BundleMetrics::increment_bundle_operation("load", "success");
                BundleMetrics::record_bundle_status("loaded");
            }
            BundleEventType::VerificationFailed => {
                error!(
                    event = event_name,
                    tag = %event.tag,
                    error = %event.error.as_deref().unwrap_or("unknown"),
                    remediation = %event.remediation_hint.as_deref().unwrap_or("none"),
                    "Bundle verification failed"
                );
                BundleMetrics::increment_bundle_operation("verify", "failed");
                BundleMetrics::increment_verification_failure("sha256_mismatch");
                BundleMetrics::record_bundle_status("verification_failed");
            }
            BundleEventType::FallbackActivated => {
                warn!(
                    event = event_name,
                    tag = %event.tag,
                    error = %event.error.as_deref().unwrap_or("unknown"),
                    remediation = %event.remediation_hint.as_deref().unwrap_or("none"),
                    "Bundle fallback activated"
                );
                BundleMetrics::increment_bundle_operation("fallback", "activated");
                BundleMetrics::record_fallback_active(true);
                BundleMetrics::record_bundle_status("using_fallback");
            }
            BundleEventType::RefreshAttempt => {
                info!(
                    event = event_name,
                    tag = %event.tag,
                    "Bundle refresh attempt started"
                );
                BundleMetrics::increment_bundle_operation("refresh", "attempt");
            }
            BundleEventType::DownloadFailed => {
                error!(
                    event = event_name,
                    tag = %event.tag,
                    error = %event.error.as_deref().unwrap_or("unknown"),
                    network_error = %event.metadata.network_error.as_deref().unwrap_or("none"),
                    remediation = %event.remediation_hint.as_deref().unwrap_or("none"),
                    "Bundle download failed"
                );
                BundleMetrics::increment_bundle_operation("download", "failed");
                BundleMetrics::record_bundle_status("download_error");
            }
            BundleEventType::StaleDetected => {
                warn!(
                    event = event_name,
                    tag = %event.tag,
                    bundle_timestamp = %event.metadata.bundle_timestamp.as_deref().unwrap_or("unknown"),
                    remediation = %event.remediation_hint.as_deref().unwrap_or("none"),
                    "Bundle is stale"
                );
                BundleMetrics::increment_bundle_operation("stale_check", "stale");
                BundleMetrics::record_bundle_status("stale");
            }
            BundleEventType::UpdateDetected => {
                info!(
                    event = event_name,
                    tag = %event.tag,
                    remediation = %event.remediation_hint.as_deref().unwrap_or("none"),
                    "Bundle update available"
                );
                BundleMetrics::increment_bundle_operation("update_check", "available");
            }
            BundleEventType::StatusCheck => {
                info!(
                    event = event_name,
                    tag = %event.tag,
                    "Bundle status check performed"
                );
                BundleMetrics::increment_bundle_operation("status_check", "completed");
            }
        }

        // Record timing if available
        if let Some(duration) = event.metadata.duration_ms {
            BundleMetrics::record_refresh_duration(duration);
        }
    }

    /// Create a loaded event
    pub fn bundle_loaded(
        tag: String,
        sha256: String,
        source: BundleSource,
        git_sha: Option<String>,
        bundle_timestamp: Option<String>,
        duration_ms: Option<u64>,
    ) -> BundleAuditEvent {
        BundleAuditEvent {
            event_type: BundleEventType::Loaded,
            tag,
            sha256: Some(sha256),
            source,
            error: None,
            remediation_hint: None,
            metadata: BundleEventMetadata {
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms,
                git_sha,
                bundle_timestamp,
                cache_path: None,
                network_error: None,
            },
        }
    }

    /// Create a verification failed event
    pub fn verification_failed(
        tag: String,
        expected_sha: String,
        actual_sha: String,
        remediation: String,
    ) -> BundleAuditEvent {
        BundleAuditEvent {
            event_type: BundleEventType::VerificationFailed,
            tag,
            sha256: Some(actual_sha.clone()),
            source: BundleSource::Download,
            error: Some(format!(
                "SHA-256 mismatch: expected {}, got {}",
                expected_sha, actual_sha
            )),
            remediation_hint: Some(remediation),
            metadata: BundleEventMetadata {
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms: None,
                git_sha: None,
                bundle_timestamp: None,
                cache_path: None,
                network_error: None,
            },
        }
    }

    /// Create a fallback activated event
    pub fn fallback_activated(tag: String, error: String, remediation: String) -> BundleAuditEvent {
        BundleAuditEvent {
            event_type: BundleEventType::FallbackActivated,
            tag,
            sha256: None,
            source: BundleSource::Fallback,
            error: Some(error),
            remediation_hint: Some(remediation),
            metadata: BundleEventMetadata {
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms: None,
                git_sha: None,
                bundle_timestamp: None,
                cache_path: None,
                network_error: None,
            },
        }
    }

    /// Create a download failed event
    pub fn download_failed(
        tag: String,
        error: String,
        network_error: Option<String>,
        remediation: String,
    ) -> BundleAuditEvent {
        BundleAuditEvent {
            event_type: BundleEventType::DownloadFailed,
            tag,
            sha256: None,
            source: BundleSource::Download,
            error: Some(error),
            remediation_hint: Some(remediation),
            metadata: BundleEventMetadata {
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms: None,
                git_sha: None,
                bundle_timestamp: None,
                cache_path: None,
                network_error,
            },
        }
    }

    /// Create a stale detected event
    pub fn stale_detected(
        tag: String,
        bundle_timestamp: String,
        age_hours: i64,
        remediation: String,
    ) -> BundleAuditEvent {
        // Record age metric
        BundleMetrics::record_bundle_age_hours(age_hours);

        BundleAuditEvent {
            event_type: BundleEventType::StaleDetected,
            tag,
            sha256: None,
            source: BundleSource::Cache,
            error: None,
            remediation_hint: Some(remediation),
            metadata: BundleEventMetadata {
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms: None,
                git_sha: None,
                bundle_timestamp: Some(bundle_timestamp),
                cache_path: None,
                network_error: None,
            },
        }
    }

    /// Create an update detected event
    pub fn update_detected(tag: String, remediation: String) -> BundleAuditEvent {
        BundleAuditEvent {
            event_type: BundleEventType::UpdateDetected,
            tag,
            sha256: None,
            source: BundleSource::Download,
            error: None,
            remediation_hint: Some(remediation),
            metadata: BundleEventMetadata {
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms: None,
                git_sha: None,
                bundle_timestamp: None,
                cache_path: None,
                network_error: None,
            },
        }
    }

    /// Create a refresh attempt event
    pub fn refresh_attempt(tag: String) -> BundleAuditEvent {
        BundleAuditEvent {
            event_type: BundleEventType::RefreshAttempt,
            tag,
            sha256: None,
            source: BundleSource::Download,
            error: None,
            remediation_hint: None,
            metadata: BundleEventMetadata {
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms: None,
                git_sha: None,
                bundle_timestamp: None,
                cache_path: None,
                network_error: None,
            },
        }
    }

    /// Create a status check event
    pub fn status_check(tag: String) -> BundleAuditEvent {
        BundleAuditEvent {
            event_type: BundleEventType::StatusCheck,
            tag,
            sha256: None,
            source: BundleSource::Cache,
            error: None,
            remediation_hint: None,
            metadata: BundleEventMetadata {
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms: None,
                git_sha: None,
                bundle_timestamp: None,
                cache_path: None,
                network_error: None,
            },
        }
    }
}
