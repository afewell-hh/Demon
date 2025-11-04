//! Telemetry module for runtime metrics and scale hints

pub mod scale_hint;

pub use scale_hint::{
    HysteresisState, PressureState, Recommendation, RuntimeMetrics, ScaleHintConfig,
    ScaleHintEmitter,
};
