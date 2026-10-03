//! Configurações globais do Terminal Pong + Laya.

use std::time::Duration;

// ============================================================================
// FÍSICA / DIMENSÕES DO CAMPO
// ============================================================================
pub const COURT_WIDTH: f64 = 70.0;
pub const COURT_HEIGHT: f64 = 24.0;
pub const PADDLE_HEIGHT: f64 = 4.5;
pub const PADDLE_WIDTH: f64 = 1.2;
pub const PADDLE_SPEED: f64 = 26.0;
pub const INITIAL_BALL_SPEED: f64 = 28.0;
pub const MAX_BALL_SPEED: f64 = 55.0;
pub const BALL_SPEED_INCREMENT: f64 = 1.04;
pub const TARGET_FPS: u64 = 60;
pub const FRAME_DURATION: Duration = Duration::from_micros(1_000_000 / TARGET_FPS);

// ============================================================================
// LAYA SERVIDOR (/v1/systemone)
// ============================================================================
pub const ENV_SERVER_URL: &str = "LAYA_SERVER_URL";
pub const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8000/v1/systemone";
/// Timeout por inferência. Laya: ~33ms GPU / 200ms-1.7s+ CPU. Configurável
/// via `LAYA_TIMEOUT_MS`.
pub const ENV_TIMEOUT_MS: &str = "LAYA_TIMEOUT_MS";
pub const DEFAULT_TIMEOUT_MS: u64 = 5000;

// ============================================================================
// LÓGICA DE CONTROLE DA IA
// ============================================================================
/// Histórico de latência para sparkline e percentis.
pub const LATENCY_HISTORY_CAPACITY: usize = 48;
/// Intervalo mínimo entre inferências com a bola se aproximando do lado da IA.
pub const MIN_INTERVAL_APPROACHING: Duration = Duration::from_millis(120);
/// Intervalo mínimo entre inferências com a bola se afastando.
pub const MIN_INTERVAL_RETREATING: Duration = Duration::from_millis(400);
/// Histerese por margem: trocamos de ação somente se a margem de decisão do
/// Laya (distância entre as 2 melhores opções) for >= ao limiar. Margem ~0
/// significa empate → mantemos a ação anterior (anti-jitter).
pub const MARGIN_SWITCH_THRESHOLD: f64 = 0.02;
/// Se o modelo insistir na mesma ação nova N inferências seguidas (mesmo com
/// margem baixa), obedecemos — convicção por repetição.
pub const MARGIN_REPEAT_FORCE: u32 = 3;
/// Backoff exponencial em erros consecutivos (base e teto).
pub const ERROR_BACKOFF_BASE: Duration = Duration::from_millis(100);
pub const ERROR_BACKOFF_MAX: Duration = Duration::from_secs(2);

// ============================================================================
// TELEMETRIA EM DISCO
// ============================================================================
pub const ENV_TELEMETRY_PATH: &str = "LAYA_TELEMETRY_PATH";
pub const DEFAULT_TELEMETRY_PATH: &str = "pong_laya_telemetry.csv";
