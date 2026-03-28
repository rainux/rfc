use std::path::PathBuf;

use crate::font::draw_text;

/// Frame buffer dimensions matching the NES output.
const WIDTH: usize = 256;
const HEIGHT: usize = 240;
const BUF_SIZE: usize = WIDTH * HEIGHT * 4;

/// Maximum number of visible ROM entries on screen.
const VISIBLE_ROWS: usize = 18;

/// A ROM entry: display name and path on disk.
#[derive(Clone, Debug)]
pub struct RomEntry {
    pub name: String,
    pub path: PathBuf,
}

/// Scan a directory for `.nes` files, returning sorted entries.
pub fn scan_roms(dir: &str) -> Vec<RomEntry> {
    let path = PathBuf::from(dir);
    let mut entries = Vec::new();

    if let Ok(read_dir) = std::fs::read_dir(&path) {
        for entry in read_dir.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nes")) {
                let name = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                entries.push(RomEntry { name, path: p });
            }
        }
    }

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries
}

/// The menu state tracks the ROM list and cursor.
pub struct Menu {
    pub roms: Vec<RomEntry>,
    pub selected: usize,
    frame_buffer: Vec<u8>,
}

impl Menu {
    pub fn new(roms: Vec<RomEntry>) -> Self {
        Self {
            roms,
            selected: 0,
            frame_buffer: vec![0u8; BUF_SIZE],
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.roms.is_empty() && self.selected < self.roms.len() - 1 {
            self.selected += 1;
        }
    }

    /// Return the currently selected ROM entry, if any.
    pub fn selected_rom(&self) -> Option<&RomEntry> {
        self.roms.get(self.selected)
    }

    /// Render the menu into the internal frame buffer and return a reference.
    pub fn render(&mut self) -> &[u8] {
        let buf = &mut self.frame_buffer;

        // Dark blue background (NES palette inspired)
        for pixel in buf.chunks_exact_mut(4) {
            pixel[0] = 0;
            pixel[1] = 0;
            pixel[2] = 40;
            pixel[3] = 255;
        }

        // Colours
        let white: [u8; 3] = [236, 238, 236];
        let grey: [u8; 3] = [152, 150, 152];
        let blue: [u8; 3] = [76, 154, 236];

        // Title
        draw_text(buf, "RFC - NES EMULATOR", 40, 16, white);

        if self.roms.is_empty() {
            draw_text(buf, "NO ROMS FOUND", 64, 100, grey);
            draw_text(buf, "PUT .NES FILES IN", 48, 120, grey);
            draw_text(buf, "THE ROMS DIRECTORY", 44, 132, grey);
        } else {
            // Scrolling window
            let visible_start = if self.selected >= VISIBLE_ROWS {
                self.selected - VISIBLE_ROWS + 1
            } else {
                0
            };

            for (vi, i) in (visible_start..).take(VISIBLE_ROWS).enumerate() {
                if i >= self.roms.len() {
                    break;
                }
                let y = 36 + vi * 10;
                let color = if i == self.selected { white } else { grey };

                // Truncate long names to fit the screen (max ~29 chars with prefix)
                let name = &self.roms[i].name;
                let max_chars = 28;
                let display_name = if name.len() > max_chars {
                    &name[..max_chars]
                } else {
                    name.as_str()
                };

                if i == self.selected {
                    let text = format!("> {display_name}");
                    draw_text(buf, &text, 16, y, color);
                } else {
                    let text = format!("  {display_name}",);
                    draw_text(buf, &text, 16, y, color);
                }
            }
        }

        // Instructions at the bottom
        draw_text(buf, "UP/DOWN:SELECT ENTER:PLAY", 16, 226, blue);

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_roms_empty_dir() {
        // A non-existent directory should return an empty list
        let roms = scan_roms("/nonexistent_path_for_test");
        assert!(roms.is_empty());
    }

    #[test]
    fn menu_navigation() {
        let roms = vec![
            RomEntry {
                name: "Game A".into(),
                path: PathBuf::from("a.nes"),
            },
            RomEntry {
                name: "Game B".into(),
                path: PathBuf::from("b.nes"),
            },
            RomEntry {
                name: "Game C".into(),
                path: PathBuf::from("c.nes"),
            },
        ];
        let mut menu = Menu::new(roms);

        assert_eq!(menu.selected, 0);
        menu.move_down();
        assert_eq!(menu.selected, 1);
        menu.move_down();
        assert_eq!(menu.selected, 2);
        menu.move_down(); // Should clamp
        assert_eq!(menu.selected, 2);
        menu.move_up();
        assert_eq!(menu.selected, 1);
        menu.move_up();
        assert_eq!(menu.selected, 0);
        menu.move_up(); // Should clamp
        assert_eq!(menu.selected, 0);
    }

    #[test]
    fn menu_render_does_not_panic() {
        let roms = vec![RomEntry {
            name: "Test ROM".into(),
            path: PathBuf::from("test.nes"),
        }];
        let mut menu = Menu::new(roms);
        let buf = menu.render();
        assert_eq!(buf.len(), 256 * 240 * 4);
    }

    #[test]
    fn menu_empty_render() {
        let mut menu = Menu::new(Vec::new());
        let buf = menu.render();
        assert_eq!(buf.len(), 256 * 240 * 4);
    }

    #[test]
    fn selected_rom_returns_correct_entry() {
        let roms = vec![
            RomEntry {
                name: "A".into(),
                path: PathBuf::from("a.nes"),
            },
            RomEntry {
                name: "B".into(),
                path: PathBuf::from("b.nes"),
            },
        ];
        let mut menu = Menu::new(roms);
        assert_eq!(menu.selected_rom().unwrap().name, "A");
        menu.move_down();
        assert_eq!(menu.selected_rom().unwrap().name, "B");
    }
}
