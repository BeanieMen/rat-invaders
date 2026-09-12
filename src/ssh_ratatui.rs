use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Result;
use russh::{
    Channel, ChannelId,
    server::{Auth, ChannelOpenHandle, Handler, Msg, Server, Session},
};

use crate::ratatui_ansii_adapter::RatatuiAdapter;

const FRAME_TIME: Duration = Duration::from_millis(1000 / 30);

pub trait ClientStateTraitBounds: Send + 'static {}
impl<T: Send + 'static> ClientStateTraitBounds for T {}

pub type RenderFunction<S> = fn(&mut Client<S>, &mut ratatui::Frame);

pub type InitStateCallback<S> =
    dyn FnOnce(&mut Client<S>, &mut ratatui::Terminal<RatatuiAdapter>) + Send;
pub type InputHandler<S> = dyn Fn(&mut Client<S>, &[u8]) + Send + Sync;

pub struct Client<S: ClientStateTraitBounds> {
    pub state: S,
    pub renderer: Arc<RenderFunction<S>>,
    pub input_handler: Arc<InputHandler<S>>,
    pub init_state_callback: Option<Box<InitStateCallback<S>>>,
    pub ratatui_terminal: Option<Arc<Mutex<ratatui::Terminal<RatatuiAdapter>>>>,
}

pub struct ClientHandler<S: ClientStateTraitBounds> {
    pub inner: Arc<Mutex<Client<S>>>,
}

pub trait SshRatatui: Server {
    type State: ClientStateTraitBounds;

    fn render(client: &mut Client<Self::State>, frame: &mut ratatui::Frame);

    fn new_client<F1, F2>(
        state: Self::State,
        init_state_callback: F1,
        input_handler: F2,
    ) -> ClientHandler<Self::State>
    where
        F1: FnOnce(&mut Client<Self::State>, &mut ratatui::Terminal<RatatuiAdapter>)
            + Send
            + 'static,
        F2: Fn(&mut Client<Self::State>, &[u8]) + Send + Sync + 'static,
    {
        let client = Client {
            state,
            renderer: Arc::new(Self::render),
            input_handler: Arc::new(input_handler),
            init_state_callback: Some(Box::new(init_state_callback)),
            ratatui_terminal: None,
        };

        ClientHandler {
            inner: Arc::new(Mutex::new(client)),
        }
    }
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
                let Ok(client) = self.inner.lock() else {
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

        if let Ok(mut client) = self.inner.try_lock() {
            let input_handler = client.input_handler.clone();
            (input_handler)(&mut client, data);
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
            let Ok(client) = self.inner.lock() else {
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

        if let Ok(mut term) = ratatui_terminal.lock() {
            term.clear()?;
        }

        let Ok(mut client) = self.inner.lock() else {
            return Ok(());
        };

        // use and throw init state_callback
        if let Some(init_state_callback) = client.init_state_callback.take()
            && let Ok(mut term) = ratatui_terminal.try_lock()
        {
            init_state_callback(&mut client, &mut term);
        }
        client.ratatui_terminal = Some(ratatui_terminal);

        let inner = self.inner.clone();

        // Background task: 30 FPS Render Loop
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

                let renderer = client.renderer.clone();

                if ratatui_terminal
                    .draw(|frame| renderer(&mut client, frame))
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
