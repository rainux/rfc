use std::path::PathBuf;

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
    pub should_launch: bool,
}

impl Menu {
    pub fn new(roms: Vec<RomEntry>) -> Self {
        Self {
            roms,
            selected: 0,
            should_launch: false,
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

    /// Render the egui menu UI.
    pub fn ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(8, 8, 30)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.heading(
                        egui::RichText::new("RFC \u{2014} NES Emulator")
                            .size(24.0)
                            .color(egui::Color32::from_rgb(236, 238, 236)),
                    );
                    ui.add_space(10.0);
                });

                ui.separator();

                if self.roms.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            egui::RichText::new("No ROM files found")
                                .size(16.0)
                                .color(egui::Color32::GRAY),
                        );
                    });
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (i, rom) in self.roms.iter().enumerate() {
                            let is_selected = i == self.selected;
                            let text =
                                egui::RichText::new(&rom.name)
                                    .size(16.0)
                                    .color(if is_selected {
                                        egui::Color32::WHITE
                                    } else {
                                        egui::Color32::from_rgb(152, 150, 152)
                                    });

                            let response = ui.selectable_label(is_selected, text);
                            if response.clicked() {
                                self.selected = i;
                                self.should_launch = true;
                            }
                            if response.double_clicked() {
                                self.selected = i;
                                self.should_launch = true;
                            }
                        }
                    });
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(
                            "\u{2191}/\u{2193} Navigate  |  Enter: Play  |  Esc: Quit",
                        )
                        .size(12.0)
                        .color(egui::Color32::from_rgb(76, 154, 236)),
                    );
                });
            });

        // Handle keyboard navigation in egui
        ctx.input(|i| {
            if i.key_pressed(egui::Key::ArrowUp) && self.selected > 0 {
                self.selected -= 1;
            }
            if i.key_pressed(egui::Key::ArrowDown)
                && !self.roms.is_empty()
                && self.selected + 1 < self.roms.len()
            {
                self.selected += 1;
            }
            if i.key_pressed(egui::Key::Enter) && !self.roms.is_empty() {
                self.should_launch = true;
            }
        });
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
