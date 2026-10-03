//! Worker assíncrono genérico: consome observações, chama o Laya e publica
//! telemetria. Uma única implementação parametrizada por `Side` (antes havia
//! dois workers 95% iguais, um por raquete).

use super::client::LayaClient;
use super::types::{AiObservation, AiStatus, AiTelemetry, Side};
use crate::config::*;
use crate::telemetry::{TelemetryEvent, TelemetrySender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{watch, RwLock};

pub async fn laya_worker_task(
    side: Side,
    server_url: String,
    inference_timeout: Duration,
    mut observation_rx: watch::Receiver<Option<AiObservation>>,
    telemetry: Arc<RwLock<AiTelemetry>>,
    events: TelemetrySender,
    running: Arc<AtomicBool>,
) {
    let client = match LayaClient::new(server_url, inference_timeout) {
        Ok(c) => c,
        Err(e) => {
            let mut tel = telemetry.write().await;
            tel.status = AiStatus::Offline(format!("Client err: {e}"));
            return;
        }
    };

    let mut consecutive_errors: u32 = 0;
    // Convicção por repetição: quantas inferências seguidas o modelo pediu a
    // mesma ação (diferente da aplicada) com margem baixa.
    let mut pending_action: Option<super::types::AiAction> = None;
    let mut pending_repeats: u32 = 0;
    // Permite inferir imediatamente na primeira observação.
    let mut last_sent = Instant::now() - MIN_INTERVAL_RETREATING;

    while running.load(Ordering::Relaxed) {
        if observation_rx.changed().await.is_err() {
            break;
        }

        let obs = match observation_rx.borrow_and_update().clone() {
            Some(o) => o,
            None => continue,
        };

        // Cadência adaptativa: infere rápido com a bola vindo; devagar quando
        // ela se afasta (economiza servidor e evita spam de decisões idênticas).
        let approaching = side.ball_approaching(obs.ball_dx);
        let min_interval = if approaching {
            MIN_INTERVAL_APPROACHING
        } else {
            MIN_INTERVAL_RETREATING
        };
        if last_sent.elapsed() < min_interval {
            continue; // a observação muda a 60fps; a próxima mudança reavalia
        }
        last_sent = Instant::now();

        {
            let mut tel = telemetry.write().await;
            if matches!(tel.status, AiStatus::Connected) {
                tel.status = AiStatus::Inferring;
            }
        }

        let start = Instant::now();
        match client.decide(&obs, side).await {
            Ok(decision) => {
                consecutive_errors = 0;
                let latency = start.elapsed();
                let latency_ms = latency.as_millis() as u64;

                let (applied_action, suppressed) = {
                    let mut tel = telemetry.write().await;

                    // Histerese pela MARGEM de decisão do Laya: se a nova
                    // decisão é diferente mas a margem é ~0 (empate entre as
                    // melhores opções), mantemos a ação anterior (anti-jitter).
                    let mut applied = tel.action;
                    let mut suppressed = false;
                    let margin = decision.margin.unwrap_or(1.0);
                    if decision.action == tel.action || margin >= MARGIN_SWITCH_THRESHOLD {
                        applied = decision.action;
                        pending_action = None;
                        pending_repeats = 0;
                    } else {
                        // Mesmo pedido repetido MARGIN_REPEAT_FORCE vezes?
                        // Força a troca mesmo com margem baixa.
                        if pending_action == Some(decision.action) {
                            pending_repeats += 1;
                        } else {
                            pending_action = Some(decision.action);
                            pending_repeats = 1;
                        }
                        if pending_repeats >= MARGIN_REPEAT_FORCE {
                            applied = decision.action;
                            pending_action = None;
                            pending_repeats = 0;
                        } else {
                            suppressed = true;
                            tel.suppressed_switches += 1;
                        }
                    }

                    tel.raw_action = decision.action;
                    tel.raw_steps = decision.steps;
                    tel.action = applied;
                    // Plano novo só vale se a direção foi efetivamente trocada;
                    // decisões suprimidas não sobrescrevem o plano em curso.
                    if applied == decision.action {
                        tel.pending_steps = decision.steps as f64;
                    }
                    tel.probability = decision.confidence;
                    tel.latency = latency;
                    tel.status = AiStatus::Connected;
                    tel.inferences_count += 1;
                    tel.last_inference_time = Instant::now();
                    tel.record_latency(latency_ms);
                    tel.last_state_summary = decision.state_summary.clone();
                    tel.last_routed_model = decision.routed_model.clone();
                    let hist_idx = match decision.action {
                        super::types::AiAction::Up => 0,
                        super::types::AiAction::Down => 1,
                        super::types::AiAction::Stay => 2,
                    };
                    tel.action_hist[hist_idx] += 1;
                    (applied, suppressed)
                };

                events
                    .send(TelemetryEvent::Inference {
                        side,
                        latency_ms,
                        raw_action: format!("{}{}", decision.action.key(), decision.steps),
                        applied_action: applied_action.key().to_string(),
                        confidence: decision.confidence.unwrap_or(f64::NAN),
                        suppressed,
                        model: decision.routed_model,
                    })
                    .ok();
            }
            Err(err) => {
                consecutive_errors = consecutive_errors.saturating_add(1);
                let msg = err.to_string();
                {
                    let mut tel = telemetry.write().await;
                    tel.status = AiStatus::Offline(msg.clone());
                    tel.error_count += 1;
                }
                events
                    .send(TelemetryEvent::Error { side, message: msg })
                    .ok();

                // Backoff exponencial com teto para não martelar o servidor.
                let backoff = ERROR_BACKOFF_BASE
                    .saturating_mul(1 << consecutive_errors.min(5))
                    .min(ERROR_BACKOFF_MAX);
                tokio::time::sleep(backoff).await;
                last_sent = Instant::now() - min_interval + Duration::from_millis(10);
            }
        }
    }
}
