use slint_term::core::{Terminal, TerminalSize};
use std::sync::{Arc, Mutex};

pub struct TerminalEmulator {
    term: Arc<Mutex<Terminal>>,
}

#[allow(dead_code)]
impl TerminalEmulator {
    pub fn new(cols: u16, rows: u16) -> Self {
        let size = TerminalSize::new(cols.max(10), rows.max(4));
        Self {
            term: Arc::new(Mutex::new(Terminal::new(size))),
        }
    }

    pub fn inner(&self) -> Arc<Mutex<Terminal>> {
        Arc::clone(&self.term)
    }

    pub fn feed(&self, bytes: &[u8]) {
        if let Ok(mut term) = self.term.lock() {
            term.feed(bytes);
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        if let Ok(mut term) = self.term.lock() {
            let size = TerminalSize::new(cols.max(10), rows.max(4));
            term.resize(size);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut term) = self.term.lock() {
            term.clear();
        }
    }

    pub fn scroll(&self, delta: i32) {
        if let Ok(mut term) = self.term.lock() {
            term.scroll(delta);
        }
    }

    pub fn drain_events(&self) -> Vec<slint_term::core::TerminalEvent> {
        if let Ok(mut term) = self.term.lock() {
            term.drain_events().collect()
        } else {
            Vec::new()
        }
    }

    pub fn size(&self) -> (u16, u16) {
        if let Ok(term) = self.term.lock() {
            let s = term.size();
            (s.cols(), s.rows())
        } else {
            (80, 24)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emulator_feed_and_resize() {
        let emu = TerminalEmulator::new(80, 24);
        assert_eq!(emu.size(), (80, 24));

        emu.feed(b"echo 'Hello World'\r\n");
        emu.scroll(2);
        emu.resize(100, 30);
        assert_eq!(emu.size(), (100, 30));

        emu.clear();
        assert_eq!(emu.size(), (100, 30));

        emu.feed(b"\x1b[6n");
        let events = emu.drain_events();
        assert_eq!(events, vec![slint_term::core::TerminalEvent::Write(b"\x1b[1;1R".to_vec())]);
    }

    #[test]
    fn test_renderer_font() {
        use slint_term::render::Renderer;
        use slint_term::core::{Cell, Ink};

        for font in [
            "CaskaydiaCove Nerd Font Propo",
            "CaskaydiaCove Nerd Font Mono",
            "CaskaydiaCove Nerd Font",
        ] {
            let mut r = Renderer::new();
            r.configure(font, 14.0, 1.0);
            println!("\nFont: {}", font);
            println!("  cell_width: {}, cell_height: {}", r.cell_width, r.cell_height);

            // Test shaping folder icon \uea83
            let cells = vec![Cell {
                text: "\u{ea83}".into(),
                width: 1,
                fg: Ink::Foreground,
                bg: Ink::Background,
                bold: false,
                italic: false,
                dim: false,
                underline: false,
                strike: false,
                inverse: false,
                hidden: false,
                selected: false,
                wrapped: false,
            }];
            let _img = r.row(&cells, None);
            let arrow_cells = vec![Cell {
                text: "\u{e0b0}".into(),
                width: 1,
                fg: Ink::Foreground,
                bg: Ink::Background,
                bold: false,
                italic: false,
                dim: false,
                underline: false,
                strike: false,
                inverse: false,
                hidden: false,
                selected: false,
                wrapped: false,
            }];
            let _img2 = r.row(&arrow_cells, None);
        }
    }
}
