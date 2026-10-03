//! Logger de telemetria em CSV, desacoplado via canal mpsc.
//!
//! Cada inferência/erro vira uma linha, permitindo análise posterior
//! (pandas, duckdb, etc.) sem impactar o game loop.

use crate::ai::Side;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

pub type TelemetrySender = mpsc::UnboundedSender<TelemetryEvent>;

#[derive(Debug)]
pub enum TelemetryEvent {
    Inference {
        side: Side,
        latency_ms: u64,
        raw_action: String,
        applied_action: String,
        confidence: f64,
        suppressed: bool,
        model: String,
    },
    Error {
        side: Side,
        message: String,
    },
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn escape_csv(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub async fn telemetry_logger_task(path: String, mut rx: mpsc::UnboundedReceiver<TelemetryEvent>) {
    let mut file = match tokio::fs::File::create(&path).await {
        Ok(f) => f,
        Err(_) => return, // logger é best-effort; jogo continua sem ele
    };

    let header = "ts_unix_ms,event,side,latency_ms,raw_action,applied_action,confidence,suppressed,model,error\n";
    if file.write_all(header.as_bytes()).await.is_err() {
        return;
    }

    while let Some(event) = rx.recv().await {
        let line = match event {
            TelemetryEvent::Inference {
                side,
                latency_ms,
                raw_action,
                applied_action,
                confidence,
                suppressed,
                model,
            } => format!(
                "{ts},inference,{side},{lat},{raw},{applied},{conf:.4},{sup},{model},\n",
                ts = now_millis(),
                side = side.label(),
                lat = latency_ms,
                raw = raw_action,
                applied = applied_action,
                conf = confidence,
                sup = suppressed,
                model = model,
            ),
            TelemetryEvent::Error { side, message } => format!(
                "{ts},error,{side},,,,,,{err}\n",
                ts = now_millis(),
                side = side.label(),
                err = escape_csv(&message),
            ),
        };
        if file.write_all(line.as_bytes()).await.is_err() {
            return;
        }
        // Flush por linha: volume baixo (~10 linhas/s), e sobrevive a crash.
        let _ = file.flush().await;
    }
}
