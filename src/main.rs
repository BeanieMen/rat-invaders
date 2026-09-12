mod ratatui_ansii_adapter;
mod ssh_ratatui;
mod game;

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use russh::{keys::PrivateKey, server::Server};
use ssh_ratatui::{Client, ClientHandler, SshRatatui};
use game::{ClientDataState, handle_input, init_state, render};

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
