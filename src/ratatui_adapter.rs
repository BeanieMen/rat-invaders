use ratatui::{
    backend::{Backend, ClearType, WindowSize},
    buffer::Cell,
    layout::{Position, Size},
    style::Color,
};
use std::io::{Error, ErrorKind::BrokenPipe};

pub struct SshBackend {
    tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    width: u16,
    height: u16,
}

impl SshBackend {
    pub fn new(
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

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
    }
}

fn fg(color: Color, output: &mut Vec<u8>) {
    match color {
        Color::Reset => output.extend_from_slice(b"\x1b[39m"),
        Color::Black => output.extend_from_slice(b"\x1b[30m"),
        Color::Red => output.extend_from_slice(b"\x1b[31m"),
        Color::Green => output.extend_from_slice(b"\x1b[32m"),
        Color::Yellow => output.extend_from_slice(b"\x1b[33m"),
        Color::Blue => output.extend_from_slice(b"\x1b[34m"),
        Color::Magenta => output.extend_from_slice(b"\x1b[35m"),
        Color::Cyan => output.extend_from_slice(b"\x1b[36m"),
        Color::Gray => output.extend_from_slice(b"\x1b[37m"),
        Color::DarkGray => output.extend_from_slice(b"\x1b[90m"),
        Color::LightRed => output.extend_from_slice(b"\x1b[91m"),
        Color::LightGreen => output.extend_from_slice(b"\x1b[92m"),
        Color::LightYellow => output.extend_from_slice(b"\x1b[93m"),
        Color::LightBlue => output.extend_from_slice(b"\x1b[94m"),
        Color::LightMagenta => output.extend_from_slice(b"\x1b[95m"),
        Color::LightCyan => output.extend_from_slice(b"\x1b[96m"),
        Color::White => output.extend_from_slice(b"\x1b[97m"),
        Color::Indexed(i) => output.extend_from_slice(format!("\x1b[38;5;{i}m").as_bytes()),
        Color::Rgb(r, g, b) => output.extend_from_slice(format!("\x1b[38;2;{r};{g};{b}m").as_bytes()),
    }
}

fn bg(color: Color, output: &mut Vec<u8>) {
    match color {
        Color::Reset => output.extend_from_slice(b"\x1b[49m"),
        Color::Black => output.extend_from_slice(b"\x1b[40m"),
        Color::Red => output.extend_from_slice(b"\x1b[41m"),
        Color::Green => output.extend_from_slice(b"\x1b[42m"),
        Color::Yellow => output.extend_from_slice(b"\x1b[43m"),
        Color::Blue => output.extend_from_slice(b"\x1b[44m"),
        Color::Magenta => output.extend_from_slice(b"\x1b[45m"),
        Color::Cyan => output.extend_from_slice(b"\x1b[46m"),
        Color::Gray => output.extend_from_slice(b"\x1b[47m"),
        Color::DarkGray => output.extend_from_slice(b"\x1b[100m"),
        Color::LightRed => output.extend_from_slice(b"\x1b[101m"),
        Color::LightGreen => output.extend_from_slice(b"\x1b[102m"),
        Color::LightYellow => output.extend_from_slice(b"\x1b[103m"),
        Color::LightBlue => output.extend_from_slice(b"\x1b[104m"),
        Color::LightMagenta => output.extend_from_slice(b"\x1b[105m"),
        Color::LightCyan => output.extend_from_slice(b"\x1b[106m"),
        Color::White => output.extend_from_slice(b"\x1b[107m"),
        Color::Indexed(i) => output.extend_from_slice(format!("\x1b[48;5;{i}m").as_bytes()),
        Color::Rgb(r, g, b) => output.extend_from_slice(format!("\x1b[48;2;{r};{g};{b}m").as_bytes()),
    }
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
            fg(cell.fg, &mut output);
            bg(cell.bg, &mut output);
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
