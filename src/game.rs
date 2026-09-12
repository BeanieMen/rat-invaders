#![allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::manual_is_multiple_of,
    clippy::too_many_lines,
    clippy::int_plus_one
)]

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{
    ratatui_ansii_adapter::RatatuiAdapter,
    ssh_ratatui::{Client},
};

pub const ENEMY_SPRITE: &[&str] = &[
    "  __         ",
    " /o \\__     ",
    "(    @\\___  ",
    " /         O ",
    "/   (_____/  ",
    "/_____/   U  ",
];

pub const PLAYER_SPRITE: &[&str] = &[
    " /\\ _ /\\   ",
    "( o . o )    ",
    " >  ^  <     ",
];

const ENEMY_W: u16 = 12;
const ENEMY_H: u16 = 6;
const PLAYER_W: u16 = 10;
const PLAYER_H: u16 = 3;

pub struct GameState {
    pub player_x: u16,
    pub bullets: Vec<(u16, u16)>,
    pub enemies: Vec<(u16, u16, bool)>,
    pub width: u16,
    pub height: u16,
    pub dir: i16,
    pub tick: u32,
    pub score: u32,
    pub game_over: bool,
}

impl GameState {
    pub fn new(width: u16, height: u16) -> Self {
        let cols = ((width.saturating_sub(6)) / (ENEMY_W + 3)).clamp(2, 6);
        let enemies = (0..2_u16)
            .flat_map(|row| {
                (0..cols).map(move |col| {
                    (col * (ENEMY_W + 3) + 3, row * (ENEMY_H + 1) + 3, true)
                })
            })
            .collect();

        Self {
            player_x: width.saturating_sub(PLAYER_W) / 2,
            bullets: vec![],
            enemies,
            width,
            height,
            dir: 1,
            tick: 0,
            score: 0,
            game_over: false,
        }
    }
}

pub fn init_state(client: &mut Client<GameState>, terminal: &mut ratatui::Terminal<RatatuiAdapter>) {
    if let Ok(size) = terminal.size() {
        client.state = GameState::new(size.width, size.height);
    }
}

pub fn handle_input(client: &mut Client<GameState>, data: &[u8]) {
    let s = &mut client.state;
    if s.game_over {
        return;
    }
    match data {
        b"a" | b"A" => s.player_x = s.player_x.saturating_sub(3),
        b"d" | b"D" => s.player_x = (s.player_x + 3).min(s.width.saturating_sub(PLAYER_W + 1)),
        b" " => {
            let bullet_x = s.player_x + PLAYER_W / 2;
            let bullet_y = s.height.saturating_sub(PLAYER_H + 2);
            s.bullets.push((bullet_x, bullet_y));
        }
        _ => {}
    }
}

#[allow(clippy::needless_pass_by_ref_mut)]
pub fn render(client: &mut Client<GameState>, frame: &mut Frame) {
    let s = &mut client.state;
    let area = frame.area();
    s.width = area.width;
    s.height = area.height;
    s.tick += 1;

    if !s.game_over {
        // Move bullets upward
        s.bullets.retain_mut(|(_, y)| {
            if *y >= 2 {
                *y -= 2;
                true
            } else {
                false
            }
        });

        // Move enemies every 10 ticks
        if s.tick % 10 == 0 {
            let dir = s.dir;
            let hits_edge = s.enemies.iter().filter(|(_, _, a)| *a).any(|(x, _, _)| {
                let next_x = *x as i16 + dir;
                next_x < 2 || (next_x + ENEMY_W as i16) >= s.width as i16 - 2
            });

            if hits_edge {
                s.dir = -s.dir;
                s.enemies.iter_mut().filter(|(_, _, a)| *a).for_each(|(_, y, _)| *y += 1);
            } else {
                s.enemies.iter_mut().filter(|(_, _, a)| *a).for_each(|(x, _, _)| *x = (*x as i16 + dir) as u16);
            }
        }

        // Collisions
        let mut dead_enemies = vec![];
        let mut dead_bullets = vec![];
        for (bi, (bx, by)) in s.bullets.iter().enumerate() {
            for (ei, (ex, ey, alive)) in s.enemies.iter().enumerate() {
                if *alive
                    && *bx >= *ex
                    && *bx < *ex + ENEMY_W
                    && *by + 1 >= *ey
                    && *by < *ey + ENEMY_H
                {
                    dead_enemies.push(ei);
                    dead_bullets.push(bi);
                }
            }
        }
        s.score += dead_enemies.len() as u32 * 100;
        for ei in dead_enemies {
            s.enemies[ei].2 = false;
        }
        dead_bullets.sort_unstable_by(|a, b| b.cmp(a));
        dead_bullets.dedup();
        for bi in dead_bullets {
            s.bullets.remove(bi);
        }

        let player_y = s.height.saturating_sub(PLAYER_H + 1);
        if s.enemies.iter().any(|(_, y, a)| *a && *y + ENEMY_H >= player_y) {
            s.game_over = true;
        }

        if s.enemies.iter().all(|(_, _, a)| !a) {
            s.game_over = true;
        }
    }

    // Outer border
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(" 🐀 RAT INVADERS 🐀 ")
            .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        area,
    );

    // HUD Header
    let hud_text = format!(" SCORE: {:05} │ CONTROLS: [A/D] Move  [Space] Fire  [Q] Quit ", s.score);
    frame.render_widget(
        Paragraph::new(Span::styled(hud_text, Style::default().fg(Color::Cyan))),
        Rect { x: 2, y: 1, width: area.width.saturating_sub(4), height: 1 },
    );

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
                Rect { x: *x, y: *y, width: ENEMY_W, height: ENEMY_H },
            );
        }
    }

    // Bullets
    for (x, y) in &s.bullets {
        if *x < area.width && *y < area.height {
            let bullet_text = vec![
                Line::from(Span::styled("║", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled("║", Style::default().fg(Color::Red))),
            ];
            frame.render_widget(
                Paragraph::new(bullet_text),
                Rect { x: *x, y: *y, width: 1, height: 2 },
            );
        }
    }

    // Player
    let player_y = s.height.saturating_sub(PLAYER_H + 1);
    if s.player_x + PLAYER_W <= area.width && player_y + PLAYER_H <= area.height {
        let text: Vec<Line> = PLAYER_SPRITE
            .iter()
            .map(|line| Line::from(Span::styled(*line, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))))
            .collect();
        frame.render_widget(
            Paragraph::new(text),
            Rect { x: s.player_x, y: player_y, width: PLAYER_W, height: PLAYER_H },
        );
    }

    // Game Over Overlay
    if s.game_over {
        let won = s.enemies.iter().all(|(_, _, a)| !a);
        let title = if won { " 🎉 VICTORY! 🎉 " } else { " 💥 GAME OVER 💥 " };
        let msg = format!("Final Score: {}\nPress Q to exit", s.score);
        let box_w = 34;
        let box_h = 5;
        let box_x = area.width.saturating_sub(box_w) / 2;
        let box_y = area.height.saturating_sub(box_h) / 2;

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_style(Style::default().fg(if won { Color::Green } else { Color::Red }).add_modifier(Modifier::BOLD));

        frame.render_widget(
            Paragraph::new(msg).block(block).style(Style::default().fg(Color::White)),
            Rect { x: box_x, y: box_y, width: box_w, height: box_h },
        );
    }
}
