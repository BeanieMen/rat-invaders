use std::{sync::Arc, time::Duration};

use anyhow::Result;
use russh::{
    Channel, ChannelId,
    server::{Auth, ChannelOpenHandle, Handler, Msg, Session},
};
use tokio::sync::Mutex;

use crate::ratatui_adapter::SshBackend;

const FRAME_TIME: Duration = Duration::from_millis(1000 / 30);

pub use russh::server::Server;

pub trait Renderer<S>: Default + Send + Sync + 'static {
    fn render(&mut self, state: &mut S, frame: &mut ratatui::Frame);
}

pub trait SshRatatui: Server {
    type State: Default + Send + 'static;
    type Renderer: Renderer<Self::State>;

    fn new_client() -> Client<Self::State, Self::Renderer> {
        Client::default()
    }
}

pub struct Client<S, R> {
    pub renderer: Arc<std::sync::Mutex<R>>,
    pub state: Arc<std::sync::Mutex<S>>,
    terminal: Option<Arc<Mutex<ratatui::Terminal<SshBackend>>>>,
}

impl<S: Default, R: Renderer<S>> Default for Client<S, R> {
    fn default() -> Self {
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

    // clippy is crying but cant do anything bout it
    #[allow(clippy::unused_async_trait_impl)]
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
    
    // clippy is crying but cant do anything bout it
    #[allow(clippy::unused_async_trait_impl)]
    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if matches!(data, b"q" | b"\x03" | b"\x04") {
            let Some(terminal) = &self.terminal else {
                return Ok(());
            };
            let Ok(mut terminal) = terminal.try_lock() else {
                return Ok(());
            };
            terminal.show_cursor().ok();
        }

        Ok(())
    }

    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        cols: u32,
        rows: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some(terminal) = &self.terminal {
            terminal.lock().await.backend_mut().resize(
                u16::try_from(cols).unwrap_or(0),
                u16::try_from(rows).unwrap_or(0),
            );
        }

        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        cols: u32,
        rows: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let ssh = session.handle();

        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                if ssh.data(channel, data).await.is_err() {
                    break;
                }
            }
        });

        let terminal = Arc::new(Mutex::new(ratatui::Terminal::new(SshBackend::new(
            tx,
            u16::try_from(cols).unwrap_or(0),
            u16::try_from(rows).unwrap_or(0),
        ))?));

        terminal.lock().await.clear()?;

        self.terminal = Some(terminal.clone());

        let renderer = self.renderer.clone();
        let state = self.state.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(FRAME_TIME);

            loop {
                interval.tick().await;

                let Ok(mut terminal) = terminal.try_lock() else {
                    continue;
                };

                let Ok(mut renderer) = renderer.try_lock() else {
                    continue;
                };

                let Ok(mut state) = state.try_lock() else {
                    continue;
                };

                terminal
                    .draw(|frame| renderer.render(&mut state, frame))
                    .ok();
            }
        });

        session.channel_success(channel)?;
        Ok(())
    }
}
