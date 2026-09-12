use std::{sync::Arc, time::Duration};

use anyhow::Result;
use russh::{
    server::{Auth, ChannelOpenHandle, Handler, Msg, Server, Session},
    Channel, ChannelId,
};
use tokio::sync::Mutex;

use crate::ratatui_adapter::SshBackend;

const FRAME_TIME: Duration = Duration::from_millis(1000 / 30);

pub trait SshRatatui: Server {
    type State: Default + Send + 'static;

    fn render(state: &mut Self::State, frame: &mut ratatui::Frame);

    fn new_client() -> Client<Self::State> {
        Client {
            renderer: Arc::new(Mutex::new(Self::render)),
            state: Arc::new(std::sync::Mutex::new(Self::State::default())),
            terminal: None,
        }
    }
}
type RenderFunction<S> = fn(&mut S, &mut ratatui::Frame);
pub struct Client<S> {
    pub renderer: Arc<Mutex<RenderFunction<S>>>,
    pub state: Arc<std::sync::Mutex<S>>,
    terminal: Option<Arc<Mutex<ratatui::Terminal<SshBackend>>>>,
}

impl<S> Handler for Client<S>
where
    S: Send + 'static,
{
    type Error = anyhow::Error;

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

    #[allow(clippy::unused_async_trait_impl)]
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if matches!(data, b"q" | b"\x03" | b"\x04") {
            if let Some(terminal) = &self.terminal {
                let Ok(mut terminal) = terminal.try_lock() else {
                    return Ok(());
                };
                terminal.show_cursor().ok();
            }
            let _ = session.close(channel);
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
            let cols = u16::try_from(cols).unwrap_or(80);
            let rows = u16::try_from(rows).unwrap_or(24);

            terminal.lock().await.backend_mut().resize(cols, rows);
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

        // Background task: SSH outbound writer
        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                if ssh.data(channel, data).await.is_err() {
                    break;
                }
            }
        });

        let initial_cols = u16::try_from(cols).unwrap_or(80);
        let initial_rows = u16::try_from(rows).unwrap_or(24);

        let terminal = Arc::new(Mutex::new(ratatui::Terminal::new(
            SshBackend::new(tx, initial_cols, initial_rows),
        )?));

        terminal.lock().await.clear()?;

        self.terminal = Some(terminal.clone());

        let renderer = self.renderer.clone();
        let state = self.state.clone();

        // Background task: 30 FPS Render Loop
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(FRAME_TIME);

            loop {
                interval.tick().await;

                let Ok(mut terminal) = terminal.try_lock() else {
                    continue;
                };

                let Ok(renderer) = renderer.try_lock() else {
                    continue;
                };

                let Ok(mut state) = state.try_lock() else {
                    continue;
                };

                if terminal.draw(|frame| renderer(&mut state, frame)).is_err() {
                    break;
                }
            }
        });

        session.channel_success(channel)?;
        Ok(())
    }
}