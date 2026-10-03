//! Renderização Ratatui: header, campo, dashboard de telemetria e footer.

use crate::ai::{AiStatus, AiTelemetry};
use crate::config::*;
use crate::game::{GameMode, GameState};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::Marker,
    text::{Line, Span},
    widgets::{
        canvas::{Canvas, Rectangle},
        Block, BorderType, Borders, Gauge, Paragraph, Sparkline, Wrap,
    },
    Frame,
};

pub fn render_ui(f: &mut Frame, game: &GameState) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header / Placar
            Constraint::Min(20),   // Corpo (Campo + Telemetria)
            Constraint::Length(3), // Footer / Controles
        ])
        .split(f.area());

    render_header(f, game, main_layout[0]);

    let body_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
        .split(main_layout[1]);

    render_court(f, game, body_layout[0]);
    render_ai_dashboard(f, game, body_layout[1]);
    render_footer(f, game, main_layout[2]);
}

fn render_header(f: &mut Frame, game: &GameState, area: Rect) {
    let (p1_label, p1_color) = match game.game_mode {
        GameMode::HumanVsAi => ("HUMANO", Color::Green),
        GameMode::AiVsAi => ("LAYA AGENT 1", Color::Cyan),
    };

    let mode_badge = match game.game_mode {
        GameMode::HumanVsAi => Span::styled(
            " [👤 HUMANO vs 🤖 LAYA] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        GameMode::AiVsAi => Span::styled(
            " [🤖 LAYA-1 vs 🤖 LAYA-2] ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let title_line = Line::from(vec![
        Span::styled(
            "⚡ RUST PONG ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        mode_badge,
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}: ", p1_label),
            Style::default().fg(p1_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:02} ", game.player_score),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("vs ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:02} ", game.ai_score),
            Style::default()
                .fg(Color::LightMagenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "LAYA AGENT 2 ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("Rally: {:02} ", game.rally_count),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:4.1} FPS ", game.fps_counter.current_fps),
            Style::default().fg(Color::LightBlue),
        ),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("CPU: {:4.1}% ", game.system_metrics.process_cpu_usage),
            Style::default().fg(Color::LightYellow),
        ),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("RAM: {:4.1}MB", game.system_metrics.process_ram_mb),
            Style::default().fg(Color::LightGreen),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    f.render_widget(
        Paragraph::new(title_line).alignment(Alignment::Center).block(block),
        area,
    );
}

fn render_court(f: &mut Frame, game: &GameState, area: Rect) {
    let pause_title = if game.is_paused { " [PAUSADO] " } else { "" };
    let court_title = format!(" Campo de Jogo ({}) {} ", game.game_mode.as_str(), pause_title);

    let left_paddle_color = match game.game_mode {
        GameMode::HumanVsAi => Color::Green,
        GameMode::AiVsAi => Color::Cyan,
    };

    let canvas = Canvas::default()
        .block(
            Block::default()
                .title(court_title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(if game.is_paused {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Cyan)
                }),
        )
        .x_bounds([0.0, COURT_WIDTH])
        .y_bounds([0.0, COURT_HEIGHT])
        .marker(Marker::Block)
        .paint(|ctx| {
            // Rede central pontilhada
            let center_x = COURT_WIDTH / 2.0;
            let mut y = 1.0;
            while y < COURT_HEIGHT {
                ctx.draw(&Rectangle {
                    x: center_x - 0.2,
                    y,
                    width: 0.4,
                    height: 1.2,
                    color: Color::DarkGray,
                });
                y += 2.4;
            }

            ctx.draw(&Rectangle {
                x: 2.0,
                y: game.player_paddle.y,
                width: PADDLE_WIDTH,
                height: PADDLE_HEIGHT,
                color: left_paddle_color,
            });

            ctx.draw(&Rectangle {
                x: COURT_WIDTH - 3.0,
                y: game.ai_paddle.y,
                width: PADDLE_WIDTH,
                height: PADDLE_HEIGHT,
                color: Color::Magenta,
            });

            for (i, &(tx, ty)) in game.ball.trail.positions.iter().enumerate() {
                let c = if i % 2 == 0 { Color::DarkGray } else { Color::Gray };
                ctx.draw(&Rectangle {
                    x: tx - 0.3,
                    y: ty - 0.3,
                    width: 0.6,
                    height: 0.6,
                    color: c,
                });
            }

            ctx.draw(&Rectangle {
                x: game.ball.x - 0.5,
                y: game.ball.y - 0.5,
                width: 1.0,
                height: 1.0,
                color: Color::Yellow,
            });
        });

    f.render_widget(canvas, area);
}

/// Painel de um agente Laya: status, ação, confiança, latências e histograma.
fn agent_panel_lines(ai: &AiTelemetry) -> Vec<Line<'static>> {
    let (col, txt) = match &ai.status {
        AiStatus::Connected => (Color::Green, "● ONLINE (laya)".to_string()),
        AiStatus::Inferring => (Color::Yellow, "● INFERINDO...".to_string()),
        AiStatus::Offline(err) => {
            let short: String = err.chars().take(12).collect();
            (Color::Red, format!("● OFFLINE ({})", short))
        }
    };

    let conf_pct = ai.probability.unwrap_or(0.0) * 100.0;
    let hist_total: u64 = ai.action_hist.iter().sum::<u64>().max(1);
    let [up_n, down_n, stay_n] = ai.action_hist;

    vec![
        Line::from(vec![
            Span::raw("Status: "),
            Span::styled(txt, Style::default().fg(col).add_modifier(Modifier::BOLD)),
            Span::raw(format!(" | erros: {}", ai.error_count)),
        ]),
        Line::from(vec![
            Span::raw("Ação: "),
            Span::styled(
                ai.action.as_str().to_string(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("  conf: {:3.0}%", conf_pct)),
        ]),
        Line::from(vec![
            Span::raw("Inferências: "),
            Span::styled(
                format!("{}", ai.inferences_count),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(format!(
                "  (suprimidas: {})",
                ai.suppressed_switches
            )),
        ]),
        Line::from(vec![
            Span::raw("Lat: "),
            Span::styled(
                format!("{}ms", ai.latency.as_millis()),
                Style::default().fg(if ai.latency.as_millis() < 150 {
                    Color::Green
                } else if ai.latency.as_millis() < 400 {
                    Color::Yellow
                } else {
                    Color::Red
                }),
            ),
            Span::raw(format!(
                "  p50:{} p95:{} p99:{}ms",
                ai.latency_percentile(0.50),
                ai.latency_percentile(0.95),
                ai.latency_percentile(0.99),
            )),
        ]),
        Line::from(format!(
            "▲{}% ▼{}% ■{}%  (modelo: {})",
            up_n * 100 / hist_total,
            down_n * 100 / hist_total,
            stay_n * 100 / hist_total,
            ai.last_routed_model,
        )),
    ]
}

fn render_ai_dashboard(f: &mut Frame, game: &GameState, area: Rect) {
    let right_ai = &game.latest_right_ai_telemetry;
    let left_ai = &game.latest_left_ai_telemetry;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // Painel esquerdo (Humano ou LAYA-1)
            Constraint::Length(7), // Painel LAYA-2 (direita)
            Constraint::Length(3), // Gauge confiança LAYA-2
            Constraint::Length(4), // Sparkline latência LAYA-2
            Constraint::Min(5),    // State / contexto enviado ao Laya - host
        ])
        .split(area);

    // 1. Painel esquerdo
    let left_lines = match game.game_mode {
        GameMode::HumanVsAi => vec![
            Line::from(vec![
                Span::raw("Controlador: "),
                Span::styled(
                    "👤 HUMANO (Teclado)",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("Input: "),
                Span::styled(
                    if game.player_paddle.target_dir > 0.0 {
                        "SUBINDO ▲"
                    } else if game.player_paddle.target_dir < 0.0 {
                        "DESCENDO ▼"
                    } else {
                        "PARADO"
                    },
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::styled("Dica: ", Style::default().fg(Color::DarkGray)),
                Span::raw("Pressione [M] ou [Tab] p/ ativar o LAYA-1"),
            ]),
        ],
        GameMode::AiVsAi => agent_panel_lines(left_ai),
    };

    let left_block = Block::default()
        .title(match game.game_mode {
            GameMode::HumanVsAi => " Raquete Esquerda (Humano) ",
            GameMode::AiVsAi => " Raquete Esquerda (LAYA-1) ",
        })
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(match game.game_mode {
            GameMode::HumanVsAi => Style::default().fg(Color::Green),
            GameMode::AiVsAi => Style::default().fg(Color::Cyan),
        });
    f.render_widget(Paragraph::new(left_lines).block(left_block), chunks[0]);

    // 2. Painel LAYA-2 (direita)
    let right_block = Block::default()
        .title(" Raquete Direita (LAYA-2) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Magenta));
    f.render_widget(
        Paragraph::new(agent_panel_lines(right_ai)).block(right_block),
        chunks[1],
    );

    // 3. Confiança calibrada do Laya (LAYA-2)
    let prob_pct = (right_ai.probability.unwrap_or(0.0) * 100.0).clamp(0.0, 100.0) as u16;
    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(" Confiança Calibrada (LAYA-2) ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .gauge_style(Style::default().fg(Color::Magenta).bg(Color::DarkGray))
        .percent(prob_pct)
        .label(format!("{}%", prob_pct));
    f.render_widget(gauge, chunks[2]);

    // 4. Sparkline de latência LAYA-2
    let lat_ms = right_ai.latency.as_millis() as u64;
    let latency_data: Vec<u64> = right_ai.latency_history.iter().copied().collect();
    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .title(format!(" Latência LAYA-2 ({}ms) ", lat_ms))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .data(&latency_data)
        .style(Style::default().fg(if lat_ms < 150 {
            Color::Green
        } else if lat_ms < 400 {
            Color::Yellow
        } else {
            Color::Red
        }));
    f.render_widget(sparkline, chunks[3]);

    // 5. State enviado + host
    let prompt_text = vec![
        Line::from(vec![
            Span::styled("Host: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(
                    "CPU: {:4.1}%  │  RAM: {:.1}/{:.1}GB",
                    game.system_metrics.global_cpu_usage,
                    game.system_metrics.used_ram_gb,
                    game.system_metrics.total_ram_gb
                ),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Endpoint: ", Style::default().fg(Color::DarkGray)),
            Span::styled(game.server_url.clone(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(Span::styled(
            "State enviado (LAYA-2):",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            right_ai.last_state_summary.clone(),
            Style::default().fg(Color::White),
        )),
    ];
    let prompt_block = Block::default()
        .title(" Contexto /v1/systemone ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(
        Paragraph::new(prompt_text)
            .wrap(Wrap { trim: true })
            .block(prompt_block),
        chunks[4],
    );
}

fn render_footer(f: &mut Frame, game: &GameState, area: Rect) {
    let mode_toggle_label = match game.game_mode {
        GameMode::HumanVsAi => "[M / Tab] Ativar LAYA vs LAYA  ",
        GameMode::AiVsAi => "[M / Tab] Assumir Controle (Manual)  ",
    };

    let controls = Line::from(vec![
        Span::styled(
            mode_toggle_label,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "[W / ↑]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Subir  "),
        Span::styled(
            "[S / ↓]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Descer  "),
        Span::styled(
            "[Espaço]",
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Pausa  "),
        Span::styled(
            "[R]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Reset  "),
        Span::styled(
            "[Q / Esc]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Sair"),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    f.render_widget(
        Paragraph::new(controls).alignment(Alignment::Center).block(block),
        area,
    );
}
