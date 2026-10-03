//! Tipos compartilhados da camada de IA: observações, ações e telemetria.

use crate::config::LATENCY_HISTORY_CAPACITY;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Lado do campo controlado por um agente Laya.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn label(&self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
        }
    }

    /// A bola está se aproximando deste lado?
    pub fn ball_approaching(&self, ball_dx: f64) -> bool {
        match self {
            Side::Left => ball_dx < 0.0,
            Side::Right => ball_dx > 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiAction {
    Up,
    Down,
    Stay,
}

impl AiAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            AiAction::Up => "SUBIR (UP)",
            AiAction::Down => "DESCER (DOWN)",
            AiAction::Stay => "MANTER (STAY)",
        }
    }

    pub fn key(&self) -> &'static str {
        match self {
            AiAction::Up => "up",
            AiAction::Down => "down",
            AiAction::Stay => "stay",
        }
    }

    pub fn from_key(s: &str) -> Self {
        let clean = s.trim().to_lowercase();
        if clean.contains("up") {
            AiAction::Up
        } else if clean.contains("down") {
            AiAction::Down
        } else {
            AiAction::Stay
        }
    }

    /// Direção de movimento aplicada à raquete (y cresce para cima no canvas).
    pub fn target_dir(&self) -> f64 {
        match self {
            AiAction::Up => 1.0,
            AiAction::Down => -1.0,
            AiAction::Stay => 0.0,
        }
    }
}

/// Snapshot imutável do estado do jogo enviado ao worker da IA.
#[derive(Clone, Debug)]
pub struct AiObservation {
    pub ball_x: f64,
    pub ball_y: f64,
    pub ball_dx: f64,
    pub ball_dy: f64,
    /// Velocidade escalar real da bola (unidades/s) — corrigido: antes o
    /// worker assumia 28.0 hardcoded mesmo com a bola acelerando.
    pub ball_speed: f64,
    pub left_paddle_y: f64,
    pub right_paddle_y: f64,
}

#[derive(Clone, Debug)]
pub enum AiStatus {
    Inferring,
    Connected,
    Offline(String),
}

/// Telemetria completa de um agente Laya.
#[derive(Clone, Debug)]
pub struct AiTelemetry {
    /// Ação efetivamente aplicada à raquete (após histerese de confiança).
    pub action: AiAction,
    /// Última ação decidida pelo modelo (antes da histerese).
    pub raw_action: AiAction,
    /// Unidades de campo ainda pendentes do último comando aplicado.
    pub pending_steps: f64,
    /// Passos pedidos na última decisão do modelo.
    pub raw_steps: u8,
    pub probability: Option<f64>,
    pub latency: Duration,
    pub status: AiStatus,
    pub inferences_count: u64,
    pub error_count: u64,
    /// Quantas vezes a histerese segurou a ação anterior.
    pub suppressed_switches: u64,
    pub last_inference_time: Instant,
    pub latency_history: VecDeque<u64>,
    /// Contagem de cada ação decidida (up, down, stay).
    pub action_hist: [u64; 3],
    pub last_state_summary: String,
    pub last_routed_model: String,
}

impl Default for AiTelemetry {
    fn default() -> Self {
        Self {
            action: AiAction::Stay,
            raw_action: AiAction::Stay,
            pending_steps: 0.0,
            raw_steps: 0,
            probability: None,
            latency: Duration::ZERO,
            status: AiStatus::Offline("Conectando...".to_string()),
            inferences_count: 0,
            error_count: 0,
            suppressed_switches: 0,
            last_inference_time: Instant::now(),
            latency_history: vec![0; LATENCY_HISTORY_CAPACITY].into(),
            action_hist: [0, 0, 0],
            last_state_summary: "Aguardando observações...".to_string(),
            last_routed_model: "-".to_string(),
        }
    }
}

impl AiTelemetry {
    /// Percentil simples sobre o histórico de latência (ms).
    pub fn latency_percentile(&self, p: f64) -> u64 {
        let mut v: Vec<u64> = self
            .latency_history
            .iter()
            .copied()
            .filter(|&x| x > 0)
            .collect();
        if v.is_empty() {
            return 0;
        }
        v.sort_unstable();
        let idx = ((v.len() as f64) * p.clamp(0.0, 1.0)).ceil() as usize;
        v[(idx.saturating_sub(1)).min(v.len() - 1)]
    }

    pub fn record_latency(&mut self, ms: u64) {
        if self.latency_history.len() >= LATENCY_HISTORY_CAPACITY {
            self.latency_history.pop_front();
        }
        self.latency_history.push_back(ms);
    }
}
