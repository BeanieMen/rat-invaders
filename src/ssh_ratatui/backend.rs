use ratatui::{
    backend::{Backend, ClearType, WindowSize},
    buffer::Cell,
    layout::{Position, Size},
    style::Color,
};
use std::io::Write;
use std::io::{Error, ErrorKind::BrokenPipe};

pub struct SshBackend {
    tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    width: u16,
    height: u16,
}

impl SshBackend {
    pub const fn new(
        tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
        width: u16,
        height: u16,
    ) -> Self {
        Self { tx, width, height }
    }

    pub fn write(&self, data: &[u8]) -> std::io::Result<()> {
        self.tx
            .send(data.to_vec())
            .map_err(|_| Error::new(BrokenPipe, "SSH writer closed"))
    }

    pub const fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
    }
}

fn color(color: Color, output: &mut Vec<u8>, foreground: bool) {
    let mode = if foreground { 38 } else { 48 };

    match color {
        Color::Reset => write!(output, "\x1b[{}m", if foreground { 39 } else { 49 }),
        Color::Indexed(i) => write!(output, "\x1b[{mode};5;{i}m"),
        Color::Rgb(r, g, b) => write!(output, "\x1b[{mode};2;{r};{g};{b}m"),
        c => {
            const ANSI_OFFSETS: [(Color, u8); 16] = [
                (Color::Black, 0),
                (Color::Red, 1),
                (Color::Green, 2),
                (Color::Yellow, 3),
                (Color::Blue, 4),
                (Color::Magenta, 5),
                (Color::Cyan, 6),
                (Color::Gray, 7),
                (Color::DarkGray, 60),
                (Color::LightRed, 61),
                (Color::LightGreen, 62),
                (Color::LightYellow, 63),
                (Color::LightBlue, 64),
                (Color::LightMagenta, 65),
                (Color::LightCyan, 66),
                (Color::White, 67),
            ];

            let offset = ANSI_OFFSETS
                .iter()
                .find(|(col, _)| *col == c)
                .map(|(_, o)| o)
                .unwrap_or(&0);
            let base = if foreground { 30 } else { 40 };
            write!(output, "\x1b[{}m", base + offset)
        }
    }
    .unwrap();
}

impl Backend for SshBackend {
    type Error = std::io::Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut output = Vec::new();

        for (x, y, cell) in content {
            output.extend_from_slice(format!("\x1b[{};{}H", y + 1, x + 1).as_bytes());

            color(cell.fg, &mut output, true);
            color(cell.bg, &mut output, false);

            output.extend_from_slice(cell.symbol().as_bytes());
        }

        self.write(&output)
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.write(b"\x1b[2J\x1b[H")
    }

    fn size(&self) -> Result<Size, Self::Error> {
        Ok(Size {
            width: self.width,
            height: self.height,
        })
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        Ok(WindowSize {
            columns_rows: Size {
                width: self.width,
                height: self.height,
            },
            pixels: Size {
                width: 0,
                height: 0,
            },
        })
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.write(b"\x1b[?25l")
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.write(b"\x1b[?25h")
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        let p = position.into();

        self.write(format!("\x1b[{};{}H", p.y + 1, p.x + 1).as_bytes())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        match clear_type {
            ClearType::All => self.write(b"\x1b[2J\x1b[H"),
            ClearType::AfterCursor => self.write(b"\x1b[0J"),
            ClearType::BeforeCursor => self.write(b"\x1b[1J"),
            ClearType::CurrentLine => self.write(b"\x1b[2K"),
            ClearType::UntilNewLine => self.write(b"\x1b[K"),
        }
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        Ok(Position { x: 0, y: 0 })
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
