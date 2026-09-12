mod game;
mod ratatui_ansii_adapter;
mod ssh_ratatui;

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Result;
use game::GameState;
use russh::{keys::PrivateKey, server::Server};
use ssh_ratatui::{Client, ClientHandler, SshRatatui};

use crate::ssh_ratatui::ClientEventHandler;

#[derive(Default)]
struct EventHandeler;

impl ClientEventHandler<GameState> for EventHandeler {
    fn handle_event(&mut self, client: &mut Client<GameState>, frame: &mut ratatui::Frame) {
        game::render(client, frame);
    }

    fn handle_input(&mut self, client: &mut Client<GameState>, input: &[u8]) {
        game::handle_input(client, input);
    }

    fn handle_init_state(
        &mut self,
        client: &mut Client<GameState>,
        terminal: &mut ratatui::Terminal<ratatui_ansii_adapter::RatatuiAdapter>,
    ) {
        game::init_state(client, terminal);
    }
}

#[derive(Default)]
struct RatatuiSshServer;

impl Server for RatatuiSshServer {
    type Handler = ClientHandler<GameState>;

    fn new_client(&mut self, _addr: Option<std::net::SocketAddr>) -> Self::Handler {
        <Self as SshRatatui>::new_client(
            GameState::new(80, 24),
            Arc::new(Mutex::new(EventHandeler)),
        )
    }
}

impl SshRatatui for RatatuiSshServer {
    type State = GameState;
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
