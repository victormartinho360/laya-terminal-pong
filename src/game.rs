//! Entidades do jogo e simulação de física.

use crate::config::*;
use crate::metrics::{FpsCounter, SystemMetrics};
use std::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// MODOS DE JOGO
// ============================================================================
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameMode {
    HumanVsAi,
    AiVsAi,
}

impl GameMode {
    pub fn toggle(&mut self) {
        *self = match self {
            GameMode::HumanVsAi => GameMode::AiVsAi,
            GameMode::AiVsAi => GameMode::HumanVsAi,
        };
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            GameMode::HumanVsAi => "HUMANO vs LAYA",
            GameMode::AiVsAi => "LAYA-1 vs LAYA-2 (AUTO)",
        }
    }
}

// ============================================================================
// RAQUETE
// ============================================================================
pub struct Paddle {
    pub y: f64,
    pub velocity: f64,
    pub target_dir: f64,
    pub decay_timer: f64,
}

impl Paddle {
    pub fn new(start_y: f64) -> Self {
        Self {
            y: start_y,
            velocity: 0.0,
            target_dir: 0.0,
            decay_timer: 0.0,
        }
    }

    pub fn update(&mut self, dt: f64) {
        if self.decay_timer > 0.0 {
            self.decay_timer -= dt;
            if self.decay_timer <= 0.0 {
                self.target_dir = 0.0;
            }
        }

        let target_vel = self.target_dir * PADDLE_SPEED;
        let blend = (30.0 * dt).min(1.0);
        self.velocity += (target_vel - self.velocity) * blend;

        self.y += self.velocity * dt;
        self.clamp();
    }

    fn clamp(&mut self) {
        let max_y = COURT_HEIGHT - PADDLE_HEIGHT;
        if self.y < 0.0 {
            self.y = 0.0;
            self.velocity = 0.0;
        } else if self.y > max_y {
            self.y = max_y;
            self.velocity = 0.0;
        }
    }

    pub fn center_y(&self) -> f64 {
        self.y + (PADDLE_HEIGHT / 2.0)
    }
}

// ============================================================================
// BOLA
// ============================================================================
pub struct BallTrail {
    pub positions: Vec<(f64, f64)>,
}

impl BallTrail {
    fn new() -> Self {
        Self {
            positions: Vec::with_capacity(8),
        }
    }

    fn push(&mut self, x: f64, y: f64) {
        if self.positions.len() >= 6 {
            self.positions.remove(0);
        }
        self.positions.push((x, y));
    }

    fn clear(&mut self) {
        self.positions.clear();
    }
}

/// Contador para variar o ângulo do saque de forma determinística
/// (antes era `self.y as i64 % 2`, sempre par → saque sempre idêntico).
static SERVE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct Ball {
    pub x: f64,
    pub y: f64,
    pub dx: f64,
    pub dy: f64,
    pub speed: f64,
    pub serve_timer: f64,
    pub trail: BallTrail,
}

impl Ball {
    pub fn new() -> Self {
        let mut b = Self {
            x: COURT_WIDTH / 2.0,
            y: COURT_HEIGHT / 2.0,
            dx: 1.0,
            dy: 0.35,
            speed: INITIAL_BALL_SPEED,
            serve_timer: 0.8,
            trail: BallTrail::new(),
        };
        b.normalize_direction();
        b
    }

    pub fn reset(&mut self, serve_towards_left: bool) {
        self.x = COURT_WIDTH / 2.0;
        self.y = COURT_HEIGHT / 2.0;
        self.speed = INITIAL_BALL_SPEED;
        self.serve_timer = 0.65;
        self.trail.clear();

        // Ângulo variado de forma pseudo-aleatória determinística.
        let n = SERVE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let angle = 0.20 + 0.10 * ((n % 4) as f64); // 0.20..0.50
        let up = (n % 2) == 0;
        self.dx = if serve_towards_left { -1.0 } else { 1.0 };
        self.dy = if up { angle } else { -angle };
        self.normalize_direction();
    }

    pub fn normalize_direction(&mut self) {
        let len = (self.dx * self.dx + self.dy * self.dy).sqrt();
        if len > 0.0001 {
            self.dx /= len;
            self.dy /= len;
        }
    }
}

// ============================================================================
// ESTADO DO JOGO
// ============================================================================

/// Plano de movimento corrente de uma raquete de IA: direção + quantas
/// unidades de campo ainda faltam percorrer (a IA decide em "passos").
#[derive(Clone, Copy, Debug)]
pub struct MotionCommand {
    pub dir: f64,
    pub remaining: f64,
}

impl Default for MotionCommand {
    fn default() -> Self {
        Self {
            dir: 0.0,
            remaining: 0.0,
        }
    }
}

pub struct GameState {
    pub player_paddle: Paddle, // Raquete Esquerda (Humano ou LAYA 1)
    pub ai_paddle: Paddle,     // Raquete Direita (LAYA 2)
    pub ball: Ball,
    pub player_score: u32,
    pub ai_score: u32,
    pub rally_count: u32,
    pub is_paused: bool,
    pub fps_counter: FpsCounter,
    pub system_metrics: SystemMetrics,
    pub latest_left_ai_telemetry: crate::ai::AiTelemetry,
    pub latest_right_ai_telemetry: crate::ai::AiTelemetry,
    pub server_url: String,
    pub game_mode: GameMode,
    /// Plano de movimento de cada raquete IA (esquerda/direita) e o último
    /// número de inferência já convertido em plano.
    pub left_motion: MotionCommand,
    pub right_motion: MotionCommand,
    pub left_consumed_inference: u64,
    pub right_consumed_inference: u64,
}

impl GameState {
    pub fn new(server_url: String) -> Self {
        let initial_paddle_y = (COURT_HEIGHT - PADDLE_HEIGHT) / 2.0;
        Self {
            player_paddle: Paddle::new(initial_paddle_y),
            ai_paddle: Paddle::new(initial_paddle_y),
            ball: Ball::new(),
            player_score: 0,
            ai_score: 0,
            rally_count: 0,
            is_paused: false,
            fps_counter: FpsCounter::new(),
            system_metrics: SystemMetrics::new(),
            latest_left_ai_telemetry: crate::ai::AiTelemetry::default(),
            latest_right_ai_telemetry: crate::ai::AiTelemetry::default(),
            server_url,
            game_mode: GameMode::HumanVsAi,
            left_motion: MotionCommand::default(),
            right_motion: MotionCommand::default(),
            left_consumed_inference: 0,
            right_consumed_inference: 0,
        }
    }
}

// ============================================================================
// SIMULAÇÃO DE FÍSICA
// ============================================================================
pub fn update_physics(game: &mut GameState, dt: f64) {
    if game.is_paused {
        return;
    }

    if game.ball.serve_timer > 0.0 {
        game.ball.serve_timer -= dt;
        return;
    }

    let ball = &mut game.ball;
    ball.trail.push(ball.x, ball.y);
    ball.x += ball.dx * ball.speed * dt;
    ball.y += ball.dy * ball.speed * dt;

    // Colisão com teto e piso
    if ball.y <= 0.5 {
        ball.y = 0.5;
        ball.dy = ball.dy.abs();
    } else if ball.y >= (COURT_HEIGHT - 0.5) {
        ball.y = COURT_HEIGHT - 0.5;
        ball.dy = -ball.dy.abs();
    }

    // Colisão com Raquete Esquerda (x = 2)
    let left_paddle_x = 2.0;
    if ball.dx < 0.0 && ball.x <= (left_paddle_x + PADDLE_WIDTH) && ball.x >= (left_paddle_x - 0.5)
    {
        let p_top = game.player_paddle.y;
        let p_bottom = p_top + PADDLE_HEIGHT;
        if ball.y >= (p_top - 0.4) && ball.y <= (p_bottom + 0.4) {
            ball.x = left_paddle_x + PADDLE_WIDTH;
            let hit_offset = (ball.y - game.player_paddle.center_y()) / (PADDLE_HEIGHT / 2.0);
            ball.dx = 1.0;
            ball.dy = hit_offset * 0.95;
            ball.speed = (ball.speed * BALL_SPEED_INCREMENT).min(MAX_BALL_SPEED);
            ball.normalize_direction();
            game.rally_count += 1;
        }
    }

    // Colisão com Raquete Direita (x = COURT_WIDTH - 3)
    let right_paddle_x = COURT_WIDTH - 3.0;
    if ball.dx > 0.0
        && ball.x >= (right_paddle_x - 0.5)
        && ball.x <= (right_paddle_x + PADDLE_WIDTH + 0.5)
    {
        let p_top = game.ai_paddle.y;
        let p_bottom = p_top + PADDLE_HEIGHT;
        if ball.y >= (p_top - 0.4) && ball.y <= (p_bottom + 0.4) {
            ball.x = right_paddle_x - 0.5;
            let hit_offset = (ball.y - game.ai_paddle.center_y()) / (PADDLE_HEIGHT / 2.0);
            ball.dx = -1.0;
            ball.dy = hit_offset * 0.95;
            ball.speed = (ball.speed * BALL_SPEED_INCREMENT).min(MAX_BALL_SPEED);
            ball.normalize_direction();
            game.rally_count += 1;
        }
    }

    // Pontuação
    if ball.x < 0.0 {
        game.ai_score += 1;
        game.rally_count = 0;
        ball.reset(true);
    } else if ball.x >= COURT_WIDTH {
        game.player_score += 1;
        game.rally_count = 0;
        ball.reset(false);
    }
}
