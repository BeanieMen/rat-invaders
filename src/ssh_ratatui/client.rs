use std::sync::{Arc, Mutex};

use super::{ClientEventHandler, ClientStateTraitBounds, SshBackend};

pub struct Client<S: ClientStateTraitBounds> {
    pub state: S,
    pub event_handler: Arc<Mutex<dyn ClientEventHandler<S>>>,
    pub ratatui_terminal: Option<Arc<Mutex<ratatui::Terminal<SshBackend>>>>,
}