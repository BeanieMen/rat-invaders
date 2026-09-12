use std::sync::{Arc, Mutex};

use russh::server::Server;

use super::{Client, ClientEventHandler, ClientHandler};
pub trait ClientStateTraitBounds: Send + Sync + 'static {}

impl<T: Send + Sync + 'static> ClientStateTraitBounds for T {}

pub trait SshRatatui: Server {
    type State: ClientStateTraitBounds;

    fn new_client(
        state: Self::State,
        event_handler: Arc<Mutex<dyn ClientEventHandler<Self::State>>>,
    ) -> ClientHandler<Self::State> {
        let client = Client {
            state,
            event_handler,
            ratatui_terminal: None,
        };

        ClientHandler {
            client: Arc::new(Mutex::new(client)),
        }
    }
}