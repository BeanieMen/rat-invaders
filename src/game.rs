use ratatui::{layout::Rect, widgets::Paragraph};

use crate::{ratatui_ansii_adapter, ssh_ratatui::Client};

#[derive(Default)]
struct Human {
    x: u16,
    y: u16,
}

#[derive(Default)]
pub struct ClientDataState {
    human: Human,
}

pub fn init_state(
    client: &mut Client<ClientDataState>,
    terminal: &mut ratatui::Terminal<ratatui_ansii_adapter::RatatuiAdapter>,
) {
    if let Ok(size) = terminal.size() {
        client.state.human.x = size.width / 2;
        client.state.human.y = size.height / 2;
    }
}

pub fn handle_input(client: &mut Client<ClientDataState>, data: &[u8]) {
    for &byte in data {
        match byte {
            b'w' | b'W' => client.state.human.y = client.state.human.y.saturating_sub(1),
            b's' | b'S' => client.state.human.y = client.state.human.y.saturating_add(1),
            b'a' | b'A' => client.state.human.x = client.state.human.x.saturating_sub(1),
            b'd' | b'D' => client.state.human.x = client.state.human.x.saturating_add(1),
            _ => {}
        }
    }
}

pub fn render(client: &Client<ClientDataState>, frame: &mut ratatui::Frame) {
    let character: &[&str] = &["a"];

    frame.render_widget(
        Paragraph::new(character),
        Rect {
            x: client.state.human.x,
            y: client.state.human.y,
            width: 1,
            height: 1,
        },
    );
}
