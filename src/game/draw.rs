#![allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{
    ENEMY_H, ENEMY_SPRITE, ENEMY_W, GameState, PLAYER_H, PLAYER_SPRITE, PLAYER_W, border,
    game_over_overlay, hud_text, tick,
};
use crate::ssh_ratatui::Client;

pub fn render(client: &mut Client<GameState>, frame: &mut Frame) {
    let s = &mut client.state;
    let area = frame.area();
    s.width = area.width;
    s.height = area.height;
    s.tick += 1;

    tick(s);

    border(frame);
    hud_text(frame, s.score);

    // Enemies
    let cols = ((s.width.saturating_sub(6)) / (ENEMY_W + 3)).clamp(2, 6);
    for (i, (x, y, alive)) in s.enemies.iter().enumerate() {
        if *alive && *x + ENEMY_W <= area.width && *y + ENEMY_H <= area.height {
            let color = match (i / cols as usize) % 3 {
                0 => Color::LightRed,
                1 => Color::LightGreen,
                _ => Color::LightMagenta,
            };
            let text: Vec<Line> = ENEMY_SPRITE
                .iter()
                .map(|line| Line::from(Span::styled(*line, Style::default().fg(color))))
                .collect();

            frame.render_widget(
                Paragraph::new(text),
                Rect {
                    x: *x,
                    y: *y,
                    width: ENEMY_W,
                    height: ENEMY_H,
                },
            );
        }
    }

    // Bullets
    for (x, y) in &s.bullets {
        if *x < area.width && *y < area.height {
            let bullet_text = vec![
                Line::from(Span::styled(
                    "║",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled("║", Style::default().fg(Color::Red))),
            ];
            frame.render_widget(
                Paragraph::new(bullet_text),
                Rect {
                    x: *x,
                    y: *y,
                    width: 1,
                    height: 2,
                },
            );
        }
    }

    // Player
    let player_y = s.height.saturating_sub(PLAYER_H + 1);
    if s.player_x + PLAYER_W <= area.width && player_y + PLAYER_H <= area.height {
        let text: Vec<Line> = PLAYER_SPRITE
            .iter()
            .map(|line| {
                Line::from(Span::styled(
                    *line,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
            })
            .collect();
        frame.render_widget(
            Paragraph::new(text),
            Rect {
                x: s.player_x,
                y: player_y,
                width: PLAYER_W,
                height: PLAYER_H,
            },
        );
    }

    // Game Over Overlay
    if s.game_over {
        let won = s.enemies.iter().all(|(_, _, a)| !a);
        game_over_overlay(frame, s.score, won);
    }
}
