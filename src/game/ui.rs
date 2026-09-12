use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
};

pub fn hud_text(frame: &mut Frame, score: u32) {
    let hud_text = format!(" SCORE: {score:05} │ CONTROLS: [A/D] Move  [Space] Fire  [Q] Quit ");
    frame.render_widget(
        Paragraph::new(Span::styled(hud_text, Style::default().fg(Color::Cyan))),
        Rect {
            x: 2,
            y: 1,
            width: frame.area().width.saturating_sub(4),
            height: 1,
        },
    );
}

pub fn border(frame: &mut Frame) {
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(" 🐀 RAT INVADERS 🐀 ")
            .title_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        frame.area(),
    );
}

pub fn game_over_overlay(frame: &mut Frame, score: u32, won: bool) {
    let area = frame.area();
    let title = if won {
        " 🎉 VICTORY! 🎉 "
    } else {
        " 💥 GAME OVER 💥 "
    };
    let msg = format!("Final Score: {score}\nPress Q to exit");
    let box_w = 34;
    let box_h = 5;
    let box_x = area.width.saturating_sub(box_w) / 2;
    let box_y = area.height.saturating_sub(box_h) / 2;

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_style(
            Style::default()
                .fg(if won { Color::Green } else { Color::Red })
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(
        Paragraph::new(msg)
            .block(block)
            .style(Style::default().fg(Color::White)),
        Rect {
            x: box_x,
            y: box_y,
            width: box_w,
            height: box_h,
        },
    );
}
