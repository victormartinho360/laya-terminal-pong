//! Cliente HTTP dedicado ao contrato Laya `POST /v1/systemone`.
//!
//! Segue a documentação oficial (https://huggingface.co/convaiinnovations/laya):
//! o request carrega um `state` (JSON arbitrário) e `questions` tipadas; a
//! resposta vem em `answers.<nome>.choice` com `confidence` calibrada.

use super::types::{AiAction, AiObservation, Side};
use crate::config::*;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt;

// ============================================================================
// RESPOSTA /v1/systemone
// ============================================================================
#[derive(Deserialize)]
struct ChoiceAnswer {
    choice: Option<String>,
    /// Probabilidade calibrada da opção escolhida — o valor útil p/ gating.
    #[serde(default, alias = "prob", alias = "probability")]
    answer_confidence: Option<f64>,
    /// Margem de decisão do Laya (distância entre as 2 melhores opções).
    /// Usada na histerese: margem ~0 = empate, mantemos a ação anterior.
    #[serde(default)]
    confidence: Option<f64>,
    /// Distribuição completa por opção; fallback se `answer_confidence` faltar.
    #[serde(default)]
    probabilities: Option<HashMap<String, f64>>,
}

#[derive(Deserialize)]
struct SystemOneResponse {
    answers: HashMap<String, ChoiceAnswer>,
    #[serde(default)]
    routing: Option<Value>,
}

// ============================================================================
// ERROS
// ============================================================================
#[derive(Debug)]
pub enum LayaError {
    Transport(reqwest::Error),
    HttpStatus(u16),
    Malformed(String),
}

impl fmt::Display for LayaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayaError::Transport(e) if e.is_timeout() => write!(f, "Timeout"),
            LayaError::Transport(e) if e.is_connect() => write!(f, "Sem conexão"),
            LayaError::Transport(e) => write!(f, "{e}"),
            LayaError::HttpStatus(code) => write!(f, "HTTP {code}"),
            LayaError::Malformed(msg) => write!(f, "JSON: {msg}"),
        }
    }
}

impl std::error::Error for LayaError {}

// ============================================================================
// CLIENTE
// ============================================================================
pub struct LayaClient {
    client: Client,
    url: String,
}

/// Resultado de uma decisão: ação, confiança calibrada, modelo roteado e o
/// state enviado (para exibição no painel).
pub struct Decision {
    pub action: AiAction,
    /// Quantas unidades de campo o modelo quer percorrer nessa direção.
    pub steps: u8,
    pub confidence: Option<f64>,
    /// Margem de decisão do Laya (0 = empate perfeito entre as 2 melhores).
    pub margin: Option<f64>,
    pub routed_model: String,
    pub state_summary: String,
}

impl LayaClient {
    pub fn new(url: String, timeout: std::time::Duration) -> Result<Self, reqwest::Error> {
        let client = Client::builder().timeout(timeout).build()?;
        Ok(Self { client, url })
    }

    /// Ponto de impacto previsto da bola no plano da raquete deste lado,
    /// considerando as paredes (espelhamento). Simplifica a decisão do modelo:
    /// em vez de física, ele só compara Y alvo com Y da raquete.
    fn predicted_impact_y(obs: &AiObservation, side: Side) -> Option<(f64, f64)> {
        let approaching = side.ball_approaching(obs.ball_dx);
        if !approaching || obs.ball_dx.abs() < 1e-6 {
            return None;
        }
        let paddle_x = match side {
            Side::Left => 2.0 + PADDLE_WIDTH,
            Side::Right => COURT_WIDTH - 3.0,
        };
        let distance = (paddle_x - obs.ball_x).abs();
        // dx/dy são normalizados; a velocidade horizontal real é dx*speed.
        let vx = obs.ball_dx.abs() * obs.ball_speed;
        if vx < 1e-3 {
            return None;
        }
        let time_to_paddle = distance / vx;
        // Integra a posição vertical com reflexão nas paredes (domínio dobrado).
        let span = COURT_HEIGHT - 1.0;
        let traveled = obs.ball_dy * obs.ball_speed * time_to_paddle;
        let raw = (obs.ball_y - 0.5) + traveled;
        let period = 2.0 * span;
        let m = raw.rem_euclid(period);
        let impact_y = 0.5 + if m <= span { m } else { period - m };
        Some((impact_y, time_to_paddle))
    }

    /// Monta o `state` JSON conforme a API Laya (dict livre).
    fn build_state(obs: &AiObservation, side: Side) -> (Value, String) {
        let (my_y, opp_y) = match side {
            Side::Right => (obs.right_paddle_y, obs.left_paddle_y),
            Side::Left => (obs.left_paddle_y, obs.right_paddle_y),
        };

        let impact = Self::predicted_impact_y(obs, side);

        let (traj_desc, impact_desc) = match impact {
            Some((y, t)) => (
                format!("ball approaching {side_label} side, impact in ~{t:.2}s", side_label = side.label(), t = t),
                format!("predicted ball y at my paddle plane is {y:.1}", y = y),
            ),
            None => (
                "ball moving away from my side".to_string(),
                "no impact predicted while ball moves away".to_string(),
            ),
        };

        let state = json!({
            "game": "pong",
            "court": { "width": COURT_WIDTH, "height": COURT_HEIGHT,
                       "note": "y=0 is the FLOOR, higher y is UP" },
            "ball": {
                "x": (obs.ball_x * 10.0).round() / 10.0,
                "y": (obs.ball_y * 10.0).round() / 10.0,
                "velocity_x": (obs.ball_dx * 100.0).round() / 100.0,
                "velocity_y": (obs.ball_dy * 100.0).round() / 100.0,
                "speed": (obs.ball_speed * 10.0).round() / 10.0,
            },
            "my_paddle": {
                "side": side.label(),
                "y_bottom": (my_y * 10.0).round() / 10.0,
                "y_center": ((my_y + PADDLE_HEIGHT / 2.0) * 10.0).round() / 10.0,
                "height": PADDLE_HEIGHT,
            },
            "opponent_paddle": {
                "y_center": ((opp_y + PADDLE_HEIGHT / 2.0) * 10.0).round() / 10.0,
            },
            "trajectory": traj_desc,
            "impact_prediction": impact_desc,
        });

        let summary = format!(
            "[{side}] bola=({bx:.1},{by:.1}) v=({dx:.2},{dy:.2})×{sp:.0} paddle_y={py:.1} | {impact}",
            side = side.label(),
            bx = obs.ball_x,
            by = obs.ball_y,
            dx = obs.ball_dx,
            dy = obs.ball_dy,
            sp = obs.ball_speed,
            py = my_y,
            impact = impact_desc,
        );

        (state, summary)
    }

    /// Pergunta tipada `choice` com critérios descritivos — é assim que o
    /// Laya sabe o que cada opção significa no sistema de coordenadas do jogo.
    /// O modelo decide direção E magnitude (passos de 1, 2 ou 3 unidades).
    fn build_questions(side: Side) -> Value {
        let side_name = side.label().to_uppercase();
        json!({
            "move": {
                "type": "choice",
                "instructions": format!(
                    "You control the {side} paddle of a Pong match. Choose the move that \
                     best brings MY paddle center onto the predicted ball y at my paddle \
                     plane, so I intercept the ball. Higher y means UP. Pick the step size \
                     by distance: 1 step for small gaps (~1-2 units), 2 for medium (~3-4), \
                     3 for large gaps (5+ units). If the ball is moving away from my side, \
                     drift toward court center y≈12 with 1 step, or stay if already near.",
                    side = side_name
                ),
                "criteria": {
                    "up3": "move UP 3 units (higher y): predicted ball y is far ABOVE my paddle center",
                    "up2": "move UP 2 units: predicted ball y is moderately ABOVE my paddle center",
                    "up1": "move UP 1 unit: predicted ball y is slightly ABOVE my paddle center",
                    "stay": "hold position: paddle center already aligned with predicted ball y",
                    "down1": "move DOWN 1 unit (lower y): predicted ball y is slightly BELOW my paddle center",
                    "down2": "move DOWN 2 units: predicted ball y is moderately BELOW my paddle center",
                    "down3": "move DOWN 3 units: predicted ball y is far BELOW my paddle center",
                }
            }
        })
    }

    /// Envia uma decisão ao Laya e converte a resposta em `Decision`.
    pub async fn decide(
        &self,
        obs: &AiObservation,
        side: Side,
    ) -> Result<Decision, LayaError> {
        let (state, state_summary) = Self::build_state(obs, side);
        let payload = json!({
            "state": state,
            "questions": Self::build_questions(side),
        });

        let response = self
            .client
            .post(&self.url)
            .json(&payload)
            .send()
            .await
            .map_err(LayaError::Transport)?;

        if !response.status().is_success() {
            return Err(LayaError::HttpStatus(response.status().as_u16()));
        }

        let parsed: SystemOneResponse = response
            .json()
            .await
            .map_err(|e| LayaError::Malformed(e.to_string()))?;

        let answer = parsed
            .answers
            .get("move")
            .ok_or_else(|| LayaError::Malformed("sem chave answers.move".to_string()))?;

        let choice = answer
            .choice
            .clone()
            .ok_or_else(|| LayaError::Malformed("answers.move.choice ausente".to_string()))?;

        let routed_model = parsed
            .routing
            .as_ref()
            .and_then(|r| r.get("model"))
            .and_then(|m| m.as_str())
            .unwrap_or("-")
            .to_string();

        // Fallback: probabilidade explícita da opção escolhida no mapa.
        let confidence = answer.answer_confidence.or_else(|| {
            let key = choice.trim().to_lowercase();
            answer
                .probabilities
                .as_ref()
                .and_then(|p| p.get(&key).copied())
        });

        // Extrai magnitude ("up3" → 3; sem dígito → 1).
        let steps = choice
            .trim()
            .chars()
            .find(|c| c.is_ascii_digit())
            .and_then(|c| c.to_digit(10))
            .map(|d| d.clamp(1, 4) as u8)
            .unwrap_or(1);

        Ok(Decision {
            action: AiAction::from_key(&choice),
            steps,
            confidence,
            margin: answer.confidence,
            routed_model,
            state_summary,
        })
    }
}
