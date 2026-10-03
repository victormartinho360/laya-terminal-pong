//! Camada de decisão via Laya: tipos, cliente HTTP `/v1/systemone` e workers.

mod client;
mod types;
mod worker;

pub use types::{AiAction, AiObservation, AiStatus, AiTelemetry, Side};
pub use worker::laya_worker_task;
