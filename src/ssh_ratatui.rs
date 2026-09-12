use std::{sync::Arc, time::Duration};

use anyhow::Result;
use russh::{
    Channel, ChannelId,
    server::{Auth, ChannelOpenHandle, Handler, Msg, Server, Session},
};
use tokio::sync::Mutex;

use crate::ratatui_ansii_adapter::RatatuiAdapter;

const FRAME_TIME: Duration = Duration::from_millis(1000 / 30);

type RenderFunction<S> = fn(&mut S, &mut ratatui::Frame);

pub trait SshRatatui: Server {
    type State: Send + 'static;

    fn render(state: &mut Self::State, frame: &mut ratatui::Frame);

    fn new_client(state: Self::State) -> Client<Self::State> {
        Client {
            renderer: Arc::new(Self::render),
            state: Arc::new(std::sync::Mutex::new(state)),
            ratatui_terminal: None,
        }
    }
}

pub struct Client<S> {
    pub renderer: Arc<RenderFunction<S>>,
    pub state: Arc<std::sync::Mutex<S>>,
    ratatui_terminal: Option<Arc<Mutex<ratatui::Terminal<RatatuiAdapter>>>>,
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
            if let Some(ratatui_terminal) = &self.ratatui_terminal {
                let Ok(mut ratatui_terminal) = ratatui_terminal.try_lock() else {
                    return Ok(());
                };
                ratatui_terminal.show_cursor().ok();
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
        if let Some(ratatui_terminal) = &self.ratatui_terminal {
            let cols = u16::try_from(cols).unwrap_or(80);
            let rows = u16::try_from(rows).unwrap_or(24);

            ratatui_terminal
                .lock()
                .await
                .backend_mut()
                .resize(cols, rows);
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

        let ratatui_terminal = Arc::new(Mutex::new(ratatui::Terminal::new(RatatuiAdapter::new(
            tx,
            initial_cols,
            initial_rows,
        ))?));

        ratatui_terminal.lock().await.clear()?;

        self.ratatui_terminal = Some(ratatui_terminal.clone());

        let renderer = self.renderer.clone();
        let state = self.state.clone();

        // Background task: 30 FPS Render Loop
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(FRAME_TIME);

            loop {
                interval.tick().await;

                let Ok(mut ratatui_terminal) = ratatui_terminal.try_lock() else {
                    continue;
                };

                let Ok(mut state) = state.try_lock() else {
                    continue;
                };

                if ratatui_terminal
                    .draw(|frame| renderer(&mut state, frame))
                    .is_err()
                {
                    break;
                }
            }
        });

        session.channel_success(channel)?;
        Ok(())
    }
}
