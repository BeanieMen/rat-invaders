#![allow(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use super::{ENEMY_H, ENEMY_W, PLAYER_H, PLAYER_W};
use crate::{ratatui_ansii_adapter::RatatuiAdapter, ssh_ratatui::Client};

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
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        let cols = ((width.saturating_sub(6)) / (ENEMY_W + 3)).clamp(2, 6);
        let enemies = (0..2_u16)
            .flat_map(|row| {
                (0..cols).map(move |col| (col * (ENEMY_W + 3) + 3, row * (ENEMY_H + 1) + 3, true))
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

pub fn init_state(
    client: &mut Client<GameState>,
    terminal: &mut ratatui::Terminal<RatatuiAdapter>,
) {
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
        b"d" | b"D" => {
            s.player_x = (s.player_x + 3).min(s.width.saturating_sub(PLAYER_W + 1));
        }
        b" " => {
            let bullet_x = s.player_x + PLAYER_W / 2;
            let bullet_y = s.height.saturating_sub(PLAYER_H + 2);
            s.bullets.push((bullet_x, bullet_y));
        }
        _ => {}
    }
}

pub fn tick(s: &mut GameState) {
    if s.game_over {
        return;
    }

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
    if s.tick.is_multiple_of(10) {
        let dir = s.dir;
        let hits_edge = s.enemies.iter().filter(|(_, _, a)| *a).any(|(x, _, _)| {
            let next_x = *x as i16 + dir;
            next_x < 2 || (next_x + ENEMY_W as i16) >= s.width as i16 - 2
        });

        if hits_edge {
            s.dir = -s.dir;
            s.enemies
                .iter_mut()
                .filter(|(_, _, a)| *a)
                .for_each(|(_, y, _)| *y += 1);
        } else {
            s.enemies
                .iter_mut()
                .filter(|(_, _, a)| *a)
                .for_each(|(x, _, _)| *x = (*x as i16 + dir) as u16);
        }
    }

    // Collisions
    let mut dead_enemies = vec![];
    let mut dead_bullets = vec![];
    for (bi, (bx, by)) in s.bullets.iter().enumerate() {
        for (ei, (ex, ey, alive)) in s.enemies.iter().enumerate() {
            if *alive && *bx >= *ex && *bx < *ex + ENEMY_W && *by + 1 >= *ey && *by < *ey + ENEMY_H
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
    if s.enemies
        .iter()
        .any(|(_, y, a)| *a && *y + ENEMY_H >= player_y)
    {
        s.game_over = true;
    }

    if s.enemies.iter().all(|(_, _, a)| !a) {
        s.game_over = true;
    }
}
