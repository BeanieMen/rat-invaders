use std::{path::PathBuf, sync::Arc};
use std::time::Duration;
use russh::keys::Algorithm;
use tokio::sync::Mutex;
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

struct SshServer;
struct Client {
    terminal: Option<Arc<Mutex<Terminal<backend::SshRatatui>>>>,
}
impl russh::server::Server for SshServer {
    type Handler = Client;

    fn new_client(&mut self, _peer_addr: Option<std::net::SocketAddr>) -> Client {
        Client { terminal: None }
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
        channel: Channel<server::Msg>,
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
        _pix_width: u32,
        _pix_height: u32,
        session: &mut Session,
    ) -> std::prelude::v1::Result<(), Self::Error> {
        let width = col_width as u16;
        let height = row_height as u16;
        println!("window change: height: {height}, width: {width}");
        if let Some(terminal) = &self.terminal {
            let mut terminal = terminal.lock().await;

            terminal.backend_mut().resize(width, height);

            terminal.resize(Rect::new(0, 0, width, height)).unwrap();
        }

        session.channel_success(channel).unwrap();

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
        _modes: &[(Pty, u32)],
        session: &mut Session,
    ) -> std::prelude::v1::Result<(), Self::Error> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

        let backend = backend::SshRatatui::new(tx, channel, col_width as u16, row_height as u16);
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
        session.channel_success(channel).unwrap();

        tokio::spawn(async move {
            let art = [
                r"  ██████╗ ███████╗ █████╗ ███╗   ██╗██╗███████╗",
                r"  ██╔══██╗██╔════╝██╔══██╗████╗  ██║██║██╔════╝",
                r"  ██████╔╝█████╗  ███████║██╔██╗ ██║██║█████╗  ",
                r"  ██╔══██╗██╔══╝  ██╔══██║██║╚██╗██║██║██╔══╝  ",
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

            let mut x: u16 = 0;

            loop {
                let mut terminal = terminal.lock().await;

                terminal
                    .draw(|frame| {
                        let mut lines = Vec::new();

                        for (row, text) in art.iter().enumerate() {
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

                            lines.push(Line::from(spans));

                            let _ = row;
                        }

                        frame.render_widget(
                            Paragraph::new(lines),
                            Rect {
                                x,
                                y: (frame.area().height - art.len() as u16) / 2 as u16,
                                width: art_width,
                                height: art.len() as u16,
                            },
                        );
                    })
                    .unwrap();

                x += 1;

                if x + art_width >= terminal.size().unwrap().width {
                    x = 0;
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });

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

        key.write_openssh_file(&path, keys::ssh_key::LineEnding::CR).unwrap();

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
