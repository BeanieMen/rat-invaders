mod ratatui_adapter;
mod ssh_ratatui;

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use russh::keys::PrivateKey;
use ssh_ratatui::{Client, Renderer, Server, SshRatatui};

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

        let art_width = u16::try_from(
            ART.iter()
                .map(|r| r.chars().count())
                .max()
                .unwrap_or_default(),
        )
        .unwrap_or_default();
      
        let art: Vec<Line> = ART
            .iter()
            .map(|row| {
                Line::from(
                    row.chars()
                        .enumerate()
                        .map(|(i, c)| {
                            Span::styled(
                                c.to_string(),
                                Style::default().fg(COLORS.get((i + usize::from(state.x)) % COLORS.len()).copied().unwrap_or(Color::Reset)),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect();

        let [_, center, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(u16::try_from(ART.len()).unwrap_or(0)),
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

#[derive(Default)]
struct RatatuiSshServer;

impl Server for RatatuiSshServer {
    type Handler = Client<ClientDataState, MyRenderer>;

    fn new_client(&mut self, _addr: Option<std::net::SocketAddr>) -> Self::Handler {
        <Self as SshRatatui>::new_client()
    }
}

impl SshRatatui for RatatuiSshServer {
    type State = ClientDataState;
    type Renderer = MyRenderer;
}

#[tokio::main]
async fn main() -> Result<()> {
    let key = PrivateKey::from(russh::keys::ssh_key::private::Ed25519Keypair::from_seed(
        &[42; 32],
    ));
    let config = russh::server::Config {
        auth_rejection_time: Duration::from_secs(0),
        keys: vec![key],
        ..Default::default()
    };

    RatatuiSshServer::run_on_address(&mut RatatuiSshServer, Arc::new(config), "0.0.0.0:2222")
        .await?;
    Ok(())
}
