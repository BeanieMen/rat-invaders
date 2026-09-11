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
    pub fn new(tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>, width: u16, height: u16) -> Self {
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

fn color(color: Color, output: &mut Vec<u8>, foreground: bool) {
    let base = if foreground { 30 } else { 40 };

    match color {
        Color::Reset => {
            output.extend_from_slice(if foreground { b"\x1b[39m" } else { b"\x1b[49m" })
        }

        Color::Black => output.extend_from_slice(format!("\x1b[{base}m").as_bytes()),
        Color::Red => output.extend_from_slice(format!("\x1b[{}m", base + 1).as_bytes()),
        Color::Green => output.extend_from_slice(format!("\x1b[{}m", base + 2).as_bytes()),
        Color::Yellow => output.extend_from_slice(format!("\x1b[{}m", base + 3).as_bytes()),
        Color::Blue => output.extend_from_slice(format!("\x1b[{}m", base + 4).as_bytes()),
        Color::Magenta => output.extend_from_slice(format!("\x1b[{}m", base + 5).as_bytes()),
        Color::Cyan => output.extend_from_slice(format!("\x1b[{}m", base + 6).as_bytes()),
        Color::Gray => output.extend_from_slice(format!("\x1b[{}m", base + 7).as_bytes()),

        Color::DarkGray => output.extend_from_slice(if foreground {
            b"\x1b[90m"
        } else {
            b"\x1b[100m"
        }),

        Color::LightRed => output.extend_from_slice(if foreground {
            b"\x1b[91m"
        } else {
            b"\x1b[101m"
        }),
        Color::LightGreen => output.extend_from_slice(if foreground {
            b"\x1b[92m"
        } else {
            b"\x1b[102m"
        }),
        Color::LightYellow => output.extend_from_slice(if foreground {
            b"\x1b[93m"
        } else {
            b"\x1b[103m"
        }),
        Color::LightBlue => output.extend_from_slice(if foreground {
            b"\x1b[94m"
        } else {
            b"\x1b[104m"
        }),
        Color::LightMagenta => output.extend_from_slice(if foreground {
            b"\x1b[95m"
        } else {
            b"\x1b[105m"
        }),
        Color::LightCyan => output.extend_from_slice(if foreground {
            b"\x1b[96m"
        } else {
            b"\x1b[106m"
        }),
        Color::White => output.extend_from_slice(if foreground {
            b"\x1b[97m"
        } else {
            b"\x1b[107m"
        }),

        Color::Indexed(i) => {
            let mode = if foreground { 38 } else { 48 };
            output.extend_from_slice(format!("\x1b[{mode};5;{i}m").as_bytes());
        }

        Color::Rgb(r, g, b) => {
            let mode = if foreground { 38 } else { 48 };
            output.extend_from_slice(format!("\x1b[{mode};2;{r};{g};{b}m").as_bytes());
        }
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
