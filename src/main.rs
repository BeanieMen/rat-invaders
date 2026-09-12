mod ratatui_ansii_adapter;
mod ssh_ratatui;

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use ratatui::{layout::Rect, widgets::Paragraph};
use russh::{keys::PrivateKey, server::Server};
use ssh_ratatui::{Client, ClientHandler, SshRatatui};

#[derive(Default)]
struct Human {
    x: u16,
    y: u16,
}

#[derive(Default)]
struct ClientDataState {
    human: Human,
}

fn init_state(
    client: &mut Client<ClientDataState>,
    terminal: &mut ratatui::Terminal<ratatui_ansii_adapter::RatatuiAdapter>,
) {
    if let Ok(size) = terminal.size() {
        client.state.human.x = size.width / 2;
        client.state.human.y = size.height / 2;
    }
}

fn handle_input(client: &mut Client<ClientDataState>, data: &[u8]) {
    for &byte in data {
        match byte {
            b'w' | b'W' => client.state.human.y = client.state.human.y.saturating_sub(1),
            b's' | b'S' => client.state.human.y = client.state.human.y.saturating_add(1),
            b'a' | b'A' => client.state.human.x = client.state.human.x.saturating_sub(1),
            b'd' | b'D' => client.state.human.x = client.state.human.x.saturating_add(1),
            _ => {}
        }
    }
}

fn render(client: &Client<ClientDataState>, frame: &mut ratatui::Frame) {
    let character: &[&str] = &["a"];

    frame.render_widget(
        Paragraph::new(character),
        Rect {
            x: client.state.human.x,
            y: client.state.human.y,
            width: 1,
            height: 1,
        },
    );
}

#[derive(Default)]
struct RatatuiSshServer;

impl Server for RatatuiSshServer {
    type Handler = ClientHandler<ClientDataState>;

    fn new_client(&mut self, _addr: Option<std::net::SocketAddr>) -> Self::Handler {
        <Self as SshRatatui>::new_client(ClientDataState::default(), init_state, handle_input)
    }
}

impl SshRatatui for RatatuiSshServer {
    type State = ClientDataState;

    fn render(client: &mut Client<Self::State>, frame: &mut ratatui::Frame) {
        render(client, frame);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let key = PrivateKey::from(
        russh::keys::ssh_key::private::Ed25519Keypair::from_seed(&[42; 32]),
    );

    let config = russh::server::Config {
        auth_rejection_time: Duration::from_secs(0),
        keys: vec![key],
        ..Default::default()
    };

    RatatuiSshServer::run_on_address(
        &mut RatatuiSshServer,
        Arc::new(config),
        "0.0.0.0:2222",
    )
    .await?;

    Ok(())
}
