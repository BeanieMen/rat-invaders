use russh::keys::Algorithm;
use std::time::{Duration, Instant};
use std::{path::PathBuf, sync::Arc};
mod backend;
use anyhow::Result;
use ratatui::{
    Terminal,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use russh::*;
use russh::{
    keys::PrivateKey,
    server::{Handler, Server, Session},
};

use tokio::sync::Mutex;

pub const FRAME_TIME: Duration = Duration::from_millis(32);
struct SshServer;
type TerminalHandle = Arc<Mutex<Terminal<backend::SshRatatui>>>;

type Render = Arc<Mutex<Box<dyn FnMut(&mut Terminal<backend::SshRatatui>) + Send>>>;

struct Client {
    terminal: Option<TerminalHandle>,
    render: Option<Render>,

    last_render: Instant,
}

impl Client {
    pub fn render(&mut self) {
        let Some(terminal) = &self.terminal else {
            return;
        };

        let Some(render) = &self.render else {
            return;
        };

        if let (Ok(mut term), Ok(mut ren)) = (terminal.try_lock(), render.try_lock()) {
            ren(&mut term);
            self.last_render = Instant::now();
        }
    }

    pub async fn window_resize_request(
        &mut self,
        _channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> std::prelude::v1::Result<(), anyhow::Error> {
        let width = col_width as u16;
        let height = row_height as u16;
        if let Some(terminal) = &self.terminal {
            let mut term = terminal.lock().await;
            term.backend_mut().resize(width, height);
            drop(term);

            self.render();
        }

        Ok(())
    }
}

impl Server for SshServer {
    type Handler = Client;

    fn new_client(&mut self, _peer_addr: Option<std::net::SocketAddr>) -> Client {
        Client {
            terminal: None,
            render: None,
            last_render: Instant::now(),
        }
    }
}

impl Handler for Client {
    type Error = anyhow::Error;

    async fn auth_none(&mut self, user: &str) -> Result<server::Auth, Self::Error> {
        println!("login: {user}");
        Ok(server::Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> std::prelude::v1::Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> std::prelude::v1::Result<(), Self::Error> {
        println!("command: {:?}", data.to_ascii_lowercase());

        session
            .data(channel, b"hello from server\r\n".as_slice())
            .unwrap();

        Ok(())
    }

    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        col_width: u32,
        row_height: u32,
        pix_width: u32,
        pix_height: u32,
        session: &mut Session,
    ) -> std::prelude::v1::Result<(), Self::Error> {
        self.window_resize_request(
            channel, col_width, row_height, pix_width, pix_height, session,
        )
        .await
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(Pty, u32)],
        session: &mut Session,
    ) -> std::prelude::v1::Result<(), Self::Error> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let backend = backend::SshRatatui::new(tx, col_width as u16, row_height as u16);
        let handle = session.handle();

        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                if let Err(e) = handle.data(channel, data).await {
                    eprintln!("SSH write failed: {e:?}");
                    break;
                }
            }
        });
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.clear().unwrap();

        let terminal = Arc::new(Mutex::new(terminal));
        self.terminal = Some(terminal.clone());

        let mut x = 0u16;
        let render: Render = Arc::new(Mutex::new(Box::new(move |terminal| {
            let art = [
                r"  ██████╗ ███████╗ █████╗ ███╗   ██╗██╗███████╗",
                r"  ██╔══██╗██╔════╝██╔══██╗████╗  ██║██║██╔════╝",
                r"  ██████╔╝█████╗  ███████║██╔██╗ ██║██║█████╗  ",
                r"  ██╔══██╗██╔══╝  ██╔══██║██║╚██╗██║██╔██╔══╝  ",
                r"  ██████╔╝███████╗██║  ██║██║ ╚████║██║███████╗",
                r"  ╚═════╝ ╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝╚═╝╚══════╝",
            ];

            let colors = [
                Color::Red,
                Color::Yellow,
                Color::Green,
                Color::Cyan,
                Color::Blue,
                Color::Magenta,
            ];

            let art_width = art.iter().map(|line| line.chars().count()).max().unwrap() as u16;

            terminal
                .draw(|frame| {
                    let lines = art
                        .iter()
                        .map(|text| {
                            let spans = text
                                .chars()
                                .enumerate()
                                .map(|(i, c)| {
                                    Span::styled(
                                        c.to_string(),
                                        Style::default()
                                            .fg(colors[(i + x as usize) % colors.len()]),
                                    )
                                })
                                .collect::<Vec<_>>();

                            Line::from(spans)
                        })
                        .collect::<Vec<_>>();

                    frame.render_widget(
                        Paragraph::new(lines),
                        Rect {
                            x,
                            y: frame.area().height.saturating_sub(art.len() as u16) / 2,
                            width: art_width,
                            height: art.len() as u16,
                        },
                    );
                })
                .unwrap();

            let width = terminal.size().unwrap().width;
            let max_x = width.saturating_sub(art_width);
            if max_x > 0 {
                x = (x + 1) % max_x;
            } else {
                x = 0;
            }
        })));

        self.render = Some(render.clone());

        let term_clone = terminal.clone();
        let render_clone = render.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(FRAME_TIME);
            loop {
                interval.tick().await;
                let mut term = term_clone.lock().await;
                let mut ren = render_clone.lock().await;
                ren(&mut term);
            }
        });
        session.channel_success(channel).unwrap();

        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let path = PathBuf::from("ed25519_key");
    let key = if path.exists() {
        PrivateKey::read_openssh_file(&path).unwrap()
    } else {
        let algorithm = Algorithm::Ed25519;
        let mut rng = russh::keys::key::safe_rng();

        let key = PrivateKey::random(&mut rng, algorithm).unwrap();

        key.write_openssh_file(&path, keys::ssh_key::LineEnding::CR)
            .unwrap();

        key
    };

    let config = russh::server::Config {
        keys: vec![key],
        ..Default::default()
    };

    let mut server = SshServer;

    server
        .run_on_address(Arc::new(config), "0.0.0.0:2222")
        .await
        .unwrap();
}



