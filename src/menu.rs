use std::path::PathBuf;

/// A file-system entry in the ROM browser: either a ROM file or a subdirectory.
#[derive(Clone, Debug)]
pub enum FsEntry {
    Dir { name: String, path: PathBuf },
    Rom { name: String, path: PathBuf },
}

impl FsEntry {
    pub fn name(&self) -> &str {
        match self {
            FsEntry::Dir { name, .. } => name,
            FsEntry::Rom { name, .. } => name,
        }
    }
}

/// Scan a directory for `.nes` files and subdirectories, returning sorted entries.
/// Directories come first (sorted), then ROM files (sorted).
pub fn scan_dir(dir: &PathBuf) -> Vec<FsEntry> {
    let mut dirs = Vec::new();
    let mut roms = Vec::new();

    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let p = entry.path();
            if p.is_dir() {
                // Skip hidden directories
                let name = p
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if !name.starts_with('.') {
                    dirs.push(FsEntry::Dir { name, path: p });
                }
            } else if p
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("nes") || e.eq_ignore_ascii_case("zip"))
            {
                let name = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                roms.push(FsEntry::Rom { name, path: p });
            }
        }
    }

    dirs.sort_by(|a, b| a.name().to_lowercase().cmp(&b.name().to_lowercase()));
    roms.sort_by(|a, b| a.name().to_lowercase().cmp(&b.name().to_lowercase()));

    let mut entries = dirs;
    entries.extend(roms);
    entries
}

/// For backward compatibility: scan and return only ROM entries as RomEntry.
#[derive(Clone, Debug)]
pub struct RomEntry {
    pub name: String,
    pub path: PathBuf,
}

pub fn scan_roms(dir: &str) -> Vec<RomEntry> {
    let path = PathBuf::from(dir);
    scan_dir(&path)
        .into_iter()
        .filter_map(|e| match e {
            FsEntry::Rom { name, path } => Some(RomEntry { name, path }),
            _ => None,
        })
        .collect()
}

/// The menu state tracks the current directory, entries, and cursor.
pub struct Menu {
    root_path: PathBuf,
    dir_stack: Vec<PathBuf>,
    pub entries: Vec<FsEntry>,
    pub selected: usize,
    pub should_launch: bool,
    /// Set when user selects a ROM — main.rs reads this path.
    pub launch_path: Option<PathBuf>,
    /// Estimated items per page, computed from window height during rendering.
    page_size: usize,
}

impl Menu {
    pub fn new(roms: Vec<RomEntry>) -> Self {
        // This constructor is kept for backward compat but now we use new_with_path
        let entries: Vec<FsEntry> = roms
            .into_iter()
            .map(|r| FsEntry::Rom {
                name: r.name,
                path: r.path,
            })
            .collect();
        Self {
            root_path: PathBuf::from("."),
            dir_stack: Vec::new(),
            entries,
            selected: 0,
            should_launch: false,
            launch_path: None,
            page_size: 15,
        }
    }

    pub fn new_with_path(root_path: &str) -> Self {
        let root = PathBuf::from(root_path);
        let entries = scan_dir(&root);
        Self {
            root_path: root.clone(),
            dir_stack: vec![root],
            entries,
            selected: 0,
            should_launch: false,
            launch_path: None,
            page_size: 15,
        }
    }

    fn current_dir(&self) -> &PathBuf {
        self.dir_stack.last().unwrap_or(&self.root_path)
    }

    fn enter_dir(&mut self, path: PathBuf) {
        self.dir_stack.push(path.clone());
        self.entries = scan_dir(&path);
        self.selected = 0;
    }

    pub fn go_back(&mut self) {
        if self.dir_stack.len() > 1 {
            self.dir_stack.pop();
            self.entries = scan_dir(self.current_dir());
            self.selected = 0;
        }
    }

    pub fn activate_selected(&mut self) {
        if let Some(entry) = self.entries.get(self.selected) {
            match entry {
                FsEntry::Dir { path, .. } => {
                    self.enter_dir(path.clone());
                }
                FsEntry::Rom { path, .. } => {
                    self.launch_path = Some(path.clone());
                    self.should_launch = true;
                }
            }
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.entries.is_empty() && self.selected < self.entries.len() - 1 {
            self.selected += 1;
        }
    }

    pub fn page_up(&mut self) {
        let page = self.page_size.max(1);
        self.selected = self.selected.saturating_sub(page);
    }

    pub fn page_down(&mut self) {
        if !self.entries.is_empty() {
            let page = self.page_size.max(1);
            self.selected = (self.selected + page).min(self.entries.len() - 1);
        }
    }

    /// Return the currently selected ROM path for launching, if any.
    pub fn selected_rom(&self) -> Option<&RomEntry> {
        // Backward compat — not used in new flow, use launch_path instead
        None
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
                    ui.add_space(4.0);

                    // Show current directory path
                    let dir_display = self.current_dir().to_string_lossy().to_string();
                    ui.label(
                        egui::RichText::new(&dir_display)
                            .size(12.0)
                            .color(egui::Color32::from_rgb(100, 100, 120)),
                    );
                    ui.add_space(6.0);
                });

                ui.separator();

                if self.entries.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            egui::RichText::new("Empty directory")
                                .size(16.0)
                                .color(egui::Color32::GRAY),
                        );
                    });
                } else {
                    // Estimate page size from available height (~22px per item)
                    let available_height = ui.available_height() - 30.0; // reserve footer
                    self.page_size = (available_height / 22.0).max(1.0) as usize;

                    // Back button if not at root
                    let has_parent = self.dir_stack.len() > 1;

                    let mut clicked_index: Option<usize> = None;
                    let mut go_back_clicked = false;

                    let scroll_id = ui.id().with("rom_scroll");
                    egui::ScrollArea::vertical()
                        .id_salt(scroll_id)
                        .show(ui, |ui| {
                            // ".." entry to go back
                            if has_parent {
                                let text = egui::RichText::new("\u{1F4C1} ..")
                                    .size(16.0)
                                    .color(egui::Color32::from_rgb(120, 160, 236));
                                if ui.selectable_label(false, text).clicked() {
                                    go_back_clicked = true;
                                    return;
                                }
                            }

                            for (i, entry) in self.entries.iter().enumerate() {
                                let is_selected = i == self.selected;
                                let (icon, color) = match entry {
                                    FsEntry::Dir { .. } => (
                                        "\u{1F4C1} ",
                                        if is_selected {
                                            egui::Color32::from_rgb(120, 200, 255)
                                        } else {
                                            egui::Color32::from_rgb(80, 140, 200)
                                        },
                                    ),
                                    FsEntry::Rom { .. } => (
                                        "",
                                        if is_selected {
                                            egui::Color32::WHITE
                                        } else {
                                            egui::Color32::from_rgb(152, 150, 152)
                                        },
                                    ),
                                };

                                let label_text = format!("{}{}", icon, entry.name());
                                let text = egui::RichText::new(&label_text).size(16.0).color(color);

                                let response = ui.selectable_label(is_selected, text);

                                // Auto-scroll to keep selected item visible
                                if is_selected {
                                    response.scroll_to_me(Some(egui::Align::Center));
                                }

                                if response.clicked() {
                                    clicked_index = Some(i);
                                }
                            }
                        });

                    // Handle deferred actions after the borrow of self.entries ends
                    if go_back_clicked {
                        self.go_back();
                    } else if let Some(i) = clicked_index {
                        self.selected = i;
                        self.activate_selected();
                    }
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    let help = if self.dir_stack.len() > 1 {
                        "\u{2191}/\u{2193} Navigate  |  Enter: Open  |  Backspace: Back  |  Esc: Quit"
                    } else {
                        "\u{2191}/\u{2193} Navigate  |  Enter: Open  |  Esc: Quit"
                    };
                    ui.label(
                        egui::RichText::new(help)
                            .size(12.0)
                            .color(egui::Color32::from_rgb(76, 154, 236)),
                    );
                });
            });

        // Handle keyboard navigation
        ctx.input(|i| {
            if i.key_pressed(egui::Key::ArrowUp) {
                self.move_up();
            }
            if i.key_pressed(egui::Key::ArrowDown) {
                self.move_down();
            }
            if i.key_pressed(egui::Key::ArrowLeft) {
                self.page_up();
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                self.page_down();
            }
            if i.key_pressed(egui::Key::Enter) && !self.entries.is_empty() {
                self.activate_selected();
            }
            if i.key_pressed(egui::Key::Backspace) {
                self.go_back();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_roms_empty_dir() {
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
        menu.move_down();
        assert_eq!(menu.selected, 2);
        menu.move_up();
        assert_eq!(menu.selected, 1);
        menu.move_up();
        assert_eq!(menu.selected, 0);
        menu.move_up();
        assert_eq!(menu.selected, 0);
    }

    #[test]
    fn scan_dir_returns_dirs_first() {
        // Test with a non-existent path — should return empty
        let entries = scan_dir(&PathBuf::from("/nonexistent_for_test"));
        assert!(entries.is_empty());
    }
}
