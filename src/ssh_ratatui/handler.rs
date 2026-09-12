use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Result;
use russh::{
    Channel, ChannelId,
    server::{Auth, ChannelOpenHandle, Handler, Msg, Session},
};

use super::{Client, ClientStateTraitBounds, SshBackend};

const FRAME_TIME: Duration = Duration::from_millis(1000 / 30);

pub trait ClientEventHandler<S>: Send + Sync + 'static
where
    S: ClientStateTraitBounds,
{
    fn handle_event(&mut self, client: &mut Client<S>, frame: &mut ratatui::Frame);

    fn handle_input(&mut self, client: &mut Client<S>, input: &[u8]);

    fn handle_init_state(
        &mut self,
        client: &mut Client<S>,
        terminal: &mut ratatui::Terminal<SshBackend>,
    );
}

pub struct ClientHandler<S: ClientStateTraitBounds> {
    pub client: Arc<Mutex<Client<S>>>,
}

impl<S: ClientStateTraitBounds> Handler for ClientHandler<S> {
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
            let ratatui_terminal = {
                let Ok(client) = self.client.lock() else {
                    return Ok(());
                };

                client.ratatui_terminal.clone()
            };

            if let Some(ratatui_terminal) = ratatui_terminal
                && let Ok(mut ratatui_terminal) = ratatui_terminal.try_lock()
            {
                ratatui_terminal.show_cursor().ok();
            }

            let _ = session.close(channel);

            return Ok(());
        }

        if let Ok(mut client) = self.client.try_lock() {
            let event_handler = client.event_handler.clone();

            event_handler
                .lock()
                .unwrap()
                .handle_input(&mut *client, data);
        }

        Ok(())
    }

    #[allow(clippy::unused_async_trait_impl)]
    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        cols: u32,
        rows: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let ratatui_terminal = {
            let Ok(client) = self.client.lock() else {
                return Ok(());
            };

            client.ratatui_terminal.clone()
        };

        if let Some(ratatui_terminal) = ratatui_terminal {
            let cols = u16::try_from(cols).unwrap_or(80);
            let rows = u16::try_from(rows).unwrap_or(24);

            if let Ok(mut ratatui_terminal) = ratatui_terminal.lock() {
                ratatui_terminal.backend_mut().resize(cols, rows);
            }
        }

        Ok(())
    }

    #[allow(clippy::unused_async_trait_impl)]
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

        let initial_cols = u16::try_from(cols).unwrap_or(80);
        let initial_rows = u16::try_from(rows).unwrap_or(24);

        let ratatui_terminal = Arc::new(Mutex::new(ratatui::Terminal::new(SshBackend::new(
            tx,
            initial_cols,
            initial_rows,
        ))?));

        if let Ok(mut terminal) = ratatui_terminal.lock() {
            terminal.clear()?;
        }

        let Ok(mut client) = self.client.lock() else {
            return Ok(());
        };

        client.ratatui_terminal = Some(ratatui_terminal.clone());

        {
            let event_handler = client.event_handler.clone();

            let mut terminal = ratatui_terminal.lock().unwrap();

            event_handler
                .lock()
                .unwrap()
                .handle_init_state(&mut client, &mut terminal);
        }

        let inner = self.client.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(FRAME_TIME);

            loop {
                interval.tick().await;

                let Ok(mut client) = inner.try_lock() else {
                    continue;
                };

                let Some(ratatui_terminal) = client.ratatui_terminal.clone() else {
                    continue;
                };

                let Ok(mut ratatui_terminal) = ratatui_terminal.try_lock() else {
                    continue;
                };

                if ratatui_terminal
                    .draw(|frame| {
                        let event_handler = client.event_handler.clone();

                        event_handler
                            .lock()
                            .unwrap()
                            .handle_event(&mut client, frame);
                    })
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
