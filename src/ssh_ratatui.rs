use std::{sync::Arc, time::Duration};

use anyhow::Result;
use russh::{
    server::{Auth, ChannelOpenHandle, Handler, Msg, Session},
    Channel, ChannelId,
};
use tokio::sync::Mutex as AsyncMutex;

use crate::ratatui_adapter::SshBackend;

// 30 fps
const FRAME_TIME: Duration = Duration::from_millis(1000 / 30);

type TerminalHandle = Arc<AsyncMutex<ratatui::Terminal<SshBackend>>>;

pub use russh::server::Server;

pub trait Renderer<S>: Default + Send + Sync + 'static {
    fn render(&mut self, state: &mut S, frame: &mut ratatui::Frame);
}

pub trait SshRatatui: Server {
    type State: Default + Send + 'static;
    type Renderer: Renderer<Self::State>;
}

pub struct Client<S, R> {
    pub renderer: Arc<std::sync::Mutex<R>>,
    pub state: Arc<std::sync::Mutex<S>>,
    terminal: Option<TerminalHandle>,
}

impl<S: Default, R: Default> Default for Client<S, R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Default, R: Default> Client<S, R> {
    pub fn new() -> Self {
        Self {
            renderer: Arc::new(std::sync::Mutex::new(R::default())),
            state: Arc::new(std::sync::Mutex::new(S::default())),
            terminal: None,
        }
    }
}

impl<S, R> Handler for Client<S, R>
where
    S: Send + 'static,
    R: Renderer<S>,
{
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