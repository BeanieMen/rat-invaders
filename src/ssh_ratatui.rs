use std::{sync::Arc, time::Duration};

use anyhow::Result;
use russh::{
    keys::ssh_key::PrivateKey,
    server::{Auth, ChannelOpenHandle, Handler, Msg, Server, Session},
    Channel, ChannelId,
};
use tokio::sync::Mutex as AsyncMutex;

use crate::ratatui_adapter::SshBackend;

const FRAME_TIME: Duration = Duration::from_millis(1000 / 30);

type TerminalHandle = Arc<AsyncMutex<ratatui::Terminal<SshBackend>>>;

pub trait Renderer<S>: Default + Send + Sync + 'static {
    fn render(&mut self, state: &mut S, frame: &mut ratatui::Frame);
}

pub trait SshRatatui: Send + Sync + 'static {
    type State: Default + Send + 'static;
    type Renderer: Renderer<Self::State>;
}

pub async fn run_server<A: SshRatatui>(bind_addr: &str) -> Result<()> {
    let key = PrivateKey::from(russh::keys::ssh_key::private::Ed25519Keypair::from_seed(
        &[42; 32],
    ));
    let config = russh::server::Config {
        auth_rejection_time: Duration::from_secs(0),
        keys: vec![key],
        ..Default::default()
    };

    SshServer::<A>(|| Client {
        renderer: Arc::new(std::sync::Mutex::new(A::Renderer::default())),
        state: Arc::new(std::sync::Mutex::new(A::State::default())),
        terminal: None,
    })
    .run_on_address(Arc::new(config), bind_addr)
    .await?;

    Ok(())
}

struct SshServer<A: SshRatatui>(fn() -> Client<A>);

impl<A: SshRatatui> Server for SshServer<A> {
    type Handler = Client<A>;

    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Client<A> {
        (self.0)()
    }
}

struct Client<A: SshRatatui> {
    renderer: Arc<std::sync::Mutex<A::Renderer>>,
    state: Arc<std::sync::Mutex<A::State>>,
    terminal: Option<TerminalHandle>,
}

impl<A: SshRatatui> Handler for Client<A> {
    type Error = anyhow::Error;

    async fn auth_none(&mut self, _user: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if matches!(data, b"q" | b"\x03" | b"\x04") {
            if let Some(t) = &self.terminal {
                if let Ok(mut term) = t.try_lock() {
                    term.show_cursor().ok();
                }
            }
        }
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some(t) = &self.terminal {
            t.lock().await.backend_mut().resize(col_width as u16, row_height as u16);
        }
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let ssh_handle = session.handle();

        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                if ssh_handle.data(channel, data).await.is_err() {
                    break;
                }
            }
        });

        let mut terminal = ratatui::Terminal::new(
            SshBackend::new(tx, col_width as u16, row_height as u16)
        )?;
        terminal.clear()?;

        let terminal_arc: TerminalHandle = Arc::new(AsyncMutex::new(terminal));
        self.terminal = Some(terminal_arc.clone());

        let renderer_ref = self.renderer.clone();
        let state_ref = self.state.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(FRAME_TIME);
            loop {
                interval.tick().await;
                if let Ok(mut term) = terminal_arc.try_lock() {
                    if let Ok(mut renderer) = renderer_ref.try_lock() {
                        if let Ok(mut state) = state_ref.try_lock() {
                            term.draw(|frame| renderer.render(&mut state, frame)).ok();
                        }
                    }
                }
            }
        });

        session.channel_success(channel)?;
        Ok(())
    }
}
