//! Processamento de input do teclado.
//!
//! Não depende de eventos de *Release* nem do protocolo Kitty: cada Press ou
//! Repeat renova um `decay_timer` curto na raquete; quando as repetições do
//! SO param, a raquete para sozinha. (Terminais que não reportam Release
//! travavam a raquete na primeira direção — esse era o bug.)

use crate::game::{GameMode, GameState};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::io;
use std::time::Duration;

/// Janela de "tecla segurada": repetições do SO costumam chegar a cada
/// 30–50ms; 150ms de decaimento é folga segura sem sensação de lag.
const KEY_HOLD_DECAY: f64 = 0.15;

pub fn process_input(game: &mut GameState) -> io::Result<bool> {
    while event::poll(Duration::ZERO)? {
        if let Event::Key(key_event) = event::read()? {
            match key_event.kind {
                KeyEventKind::Press | KeyEventKind::Repeat => match key_event.code {
                    KeyCode::Char('m') | KeyCode::Char('M') | KeyCode::Tab
                        if key_event.kind == KeyEventKind::Press =>
                    {
                        game.game_mode.toggle();
                    }
                    KeyCode::Char('w') | KeyCode::Char('W') | KeyCode::Up => {
                        if game.game_mode == GameMode::HumanVsAi {
                            game.player_paddle.target_dir = 1.0;
                            game.player_paddle.decay_timer = KEY_HOLD_DECAY;
                        }
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Down => {
                        if game.game_mode == GameMode::HumanVsAi {
                            game.player_paddle.target_dir = -1.0;
                            game.player_paddle.decay_timer = KEY_HOLD_DECAY;
                        }
                    }
                    KeyCode::Char(' ') if key_event.kind == KeyEventKind::Press => {
                        game.is_paused = !game.is_paused;
                    }
                    KeyCode::Char('r') | KeyCode::Char('R')
                        if key_event.kind == KeyEventKind::Press =>
                    {
                        game.player_score = 0;
                        game.ai_score = 0;
                        game.rally_count = 0;
                        game.ball.reset(false);
                    }
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc
                        if key_event.kind == KeyEventKind::Press =>
                    {
                        return Ok(false);
                    }
                    _ => {}
                },
                // Terminais com Kitty keyboard enhancement: para imediatamente.
                KeyEventKind::Release => match key_event.code {
                    KeyCode::Char('w') | KeyCode::Char('W') | KeyCode::Up
                    | KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Down => {
                        game.player_paddle.decay_timer = 0.0;
                        game.player_paddle.target_dir = 0.0;
                    }
                    _ => {}
                },
            }
        }
    }

    Ok(true)
}
