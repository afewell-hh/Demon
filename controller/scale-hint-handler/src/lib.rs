//! Scale Hint Handler - consumes agent.scale.hint events and triggers autoscale actions
//!
//! This service subscribes to scale hint events from NATS JetStream and provides
//! pluggable autoscaling integrations. By default it logs recommendations, but can
//! optionally call external autoscale APIs (e.g., Kubernetes HPA, cloud autoscalers).

pub mod autoscale;
pub mod config;
pub mod consumer;
pub mod metrics;

pub use autoscale::{AutoscaleClient, HttpAutoscaleClient, LogOnlyAutoscaleClient};
pub use config::Config;
pub use consumer::ScaleHintConsumer;
pub use metrics::Metrics;
