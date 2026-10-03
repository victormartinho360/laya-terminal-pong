//! Terminal Pong com decisões via Laya (convaiinnovations/laya).
//!
//! Arquitetura:
//! - `game`      — entidades e física
//! - `metrics`   — FPS e recursos do sistema
//! - `ai`        — observações, cliente `/v1/systemone` e workers por lado
//! - `telemetry` — logger CSV desacoplado
//! - `ui`        — renderização Ratatui
//! - `input`     — teclado não-bloqueante

mod ai;
mod config;
mod game;
mod input;
mod metrics;
mod telemetry;
mod ui;

use crossterm::{
    cursor::{Hide, Show},
    event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    env, io, panic,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch, RwLock};

use ai::{AiObservation, AiTelemetry, Side};
use config::*;
use game::{GameMode, GameState};

// ============================================================================
// RAII TERMINAL GUARD
// ============================================================================
struct TerminalGuard;

impl TerminalGuard {
    fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        let _ = execute!(
            stdout,
            EnterAlternateScreen,
            Hide,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::REPORT_EVENT_TYPES)
        );

        let default_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let mut stdout = io::stdout();
            let _ = execute!(
                stdout,
                PopKeyboardEnhancementFlags,
                LeaveAlternateScreen,
                Show
            );
            let _ = disable_raw_mode();
            default_hook(info);
        }));

        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(
            stdout,
            PopKeyboardEnhancementFlags,
            LeaveAlternateScreen,
            Show
        );
        let _ = disable_raw_mode();
    }
}

// ============================================================================
// CONTROLE DAS RAQUETES IA — EXCLUSIVAMENTE PELO LAYA (sem fallback)
// ============================================================================
/// Converte a última decisão da IA em um plano de movimento (direção +
/// unidades de campo restantes) e executa um passo da física da raquete.
///
/// O plano só é renovado quando chega uma inferência nova (telemetria muda
/// o `inferences_count`), então a raquete percorre os passos pedidos mesmo
/// entre decisões — e nunca "trava" numa direção.
fn apply_ai_paddle(
    paddle: &mut game::Paddle,
    motion: &mut game::MotionCommand,
    consumed: &mut u64,
    telemetry: &AiTelemetry,
    dt: f64,
) {
    if telemetry.inferences_count > *consumed {
        *consumed = telemetry.inferences_count;
        motion.dir = telemetry.action.target_dir();
        motion.remaining = if telemetry.pending_steps > 0.0 {
            telemetry.pending_steps
        } else {
            0.0
        };
        // STAY: zera qualquer movimento pendente imediatamente.
        if telemetry.action == ai::AiAction::Stay {
            motion.remaining = 0.0;
        }
    }

    let prev_y = paddle.y;
    paddle.target_dir = if motion.remaining > 0.0 {
        motion.dir
    } else {
        0.0
    };
    paddle.update(dt);
    motion.remaining = (motion.remaining - (paddle.y - prev_y).abs()).max(0.0);
}

// ============================================================================
// MAIN
// ============================================================================
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TerminalGuard::new()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let server_url =
        env::var(ENV_SERVER_URL).unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string());
    let telemetry_path =
        env::var(ENV_TELEMETRY_PATH).unwrap_or_else(|_| DEFAULT_TELEMETRY_PATH.to_string());
    let inference_timeout = Duration::from_millis(
        env::var(ENV_TIMEOUT_MS)
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_TIMEOUT_MS),
    );

    // Canais de observação (watch = sempre o estado mais recente)
    let (observation_tx, observation_rx_right) = watch::channel::<Option<AiObservation>>(None);
    let observation_rx_left = observation_tx.subscribe();

    // Canal de eventos de telemetria → logger CSV
    let (events_tx, events_rx) = mpsc::unbounded_channel();

    let ai_right_telemetry = Arc::new(RwLock::new(AiTelemetry::default()));
    let ai_left_telemetry = Arc::new(RwLock::new(AiTelemetry::default()));
    let running = Arc::new(AtomicBool::new(true));

    // Workers Laya (direita sempre ativa; esquerda usada no modo AI vs AI)
    let right_worker = tokio::spawn(ai::laya_worker_task(
        Side::Right,
        server_url.clone(),
        inference_timeout,
        observation_rx_right,
        Arc::clone(&ai_right_telemetry),
        events_tx.clone(),
        Arc::clone(&running),
    ));
    let left_worker = tokio::spawn(ai::laya_worker_task(
        Side::Left,
        server_url.clone(),
        inference_timeout,
        observation_rx_left,
        Arc::clone(&ai_left_telemetry),
        events_tx.clone(),
        Arc::clone(&running),
    ));
    let logger = tokio::spawn(telemetry::telemetry_logger_task(telemetry_path, events_rx));
    drop(events_tx); // workers e logger seguram clones próprios

    let mut game = GameState::new(server_url);

    let mut last_frame_time = Instant::now();
    let mut interval = tokio::time::interval(FRAME_DURATION);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    while running.load(Ordering::Relaxed) {
        interval.tick().await;

        let now = Instant::now();
        let dt = now.duration_since(last_frame_time).as_secs_f64().min(0.05);
        last_frame_time = now;

        game.fps_counter.tick();
        game.system_metrics.tick();

        if !input::process_input(&mut game)? {
            running.store(false, Ordering::Relaxed);
            break;
        }

        // Leitura não-bloqueante das telemetrias
        if let Ok(tel) = ai_right_telemetry.try_read() {
            game.latest_right_ai_telemetry = tel.clone();
        }
        if let Ok(tel) = ai_left_telemetry.try_read() {
            game.latest_left_ai_telemetry = tel.clone();
        }

        // Snapshot de observação para ambos os agentes
        let obs = AiObservation {
            ball_x: game.ball.x,
            ball_y: game.ball.y,
            ball_dx: game.ball.dx,
            ball_dy: game.ball.dy,
            ball_speed: game.ball.speed,
            left_paddle_y: game.player_paddle.y,
            right_paddle_y: game.ai_paddle.y,
        };
        let _ = observation_tx.send_replace(Some(obs));

        // Raquete esquerda: humano ou LAYA-1
        match game.game_mode {
            GameMode::HumanVsAi => game.player_paddle.update(dt),
            GameMode::AiVsAi => {
                let tel = game.latest_left_ai_telemetry.clone();
                apply_ai_paddle(
                    &mut game.player_paddle,
                    &mut game.left_motion,
                    &mut game.left_consumed_inference,
                    &tel,
                    dt,
                );
            }
        }

        // Raquete direita: sempre LAYA-2
        let tel = game.latest_right_ai_telemetry.clone();
        apply_ai_paddle(
            &mut game.ai_paddle,
            &mut game.right_motion,
            &mut game.right_consumed_inference,
            &tel,
            dt,
        );

        game::update_physics(&mut game, dt);
        terminal.draw(|f| ui::render_ui(f, &game))?;
    }

    // Encerramento limpo
    running.store(false, Ordering::Relaxed);
    let _ = observation_tx.send_replace(None);
    let _ = tokio::time::timeout(Duration::from_millis(100), right_worker).await;
    let _ = tokio::time::timeout(Duration::from_millis(100), left_worker).await;
    let _ = tokio::time::timeout(Duration::from_millis(200), logger).await;

    Ok(())
}
