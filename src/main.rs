mod ssh_ratatui;
mod ratatui_adapter;

use ssh_ratatui::{Renderer, SshRatatui, run_server};
use anyhow::Result;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

#[derive(Default)]
struct ClientDataState {
    x: u16,
}

#[derive(Default)]
struct MyRenderer;

impl Renderer<ClientDataState> for MyRenderer {
    fn render(&mut self, state: &mut ClientDataState, frame: &mut ratatui::Frame) {
        const COLORS: [Color; 6] = [
            Color::Red,
            Color::Yellow,
            Color::Green,
            Color::Cyan,
            Color::Blue,
            Color::Magenta,
        ];

        const ART: &[&str] = &[
            "  ██████╗ ███████╗ █████╗ ███╗   ██╗██╗███████╗",
            "  ██╔══██╗██╔════╝██╔══██╗████╗  ██║██║██╔════╝",
            "  ██████╔╝█████╗  ███████║██╔██╗ ██║██║█████╗  ",
            "  ██╔══██╗██╔══╝  ██╔══██║██║╚██╗██║██║██╔══╝  ",
            "  ██████╔╝███████╗██║  ██║██║ ╚████║██║███████╗",
            "  ╚═════╝ ╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝╚═╝╚══════╝",
        ];

        let art_width = ART.iter().map(|r| r.chars().count()).max().unwrap_or(0) as u16;

        let art: Vec<Line> = ART
            .iter()
            .map(|row| {
                Line::from(
                    row.chars()
                        .enumerate()
                        .map(|(i, c)| {
                            Span::styled(
                                c.to_string(),
                                Style::default().fg(COLORS[(i + state.x as usize) % COLORS.len()]),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect();

        let [_, center, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(ART.len() as u16),
            Constraint::Fill(1),
        ])
        .areas(frame.area());

        frame.render_widget(
            Paragraph::new(art),
            Rect {
                x: state.x,
                ..center
            },
        );

        let max_x = frame.area().width.saturating_sub(art_width);
        state.x = if max_x > 0 { (state.x + 1) % max_x } else { 0 };
    }
}

struct RatatuiSshServer;

impl SshRatatui for RatatuiSshServer {
    type State = ClientDataState;
    type Renderer = MyRenderer;
}

#[tokio::main]
async fn main() -> Result<()> {
    run_server::<RatatuiSshServer>("0.0.0.0:2222").await
}
