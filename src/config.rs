use serde::Deserialize;
use std::path::PathBuf;
use winit::keyboard::KeyCode;

use crate::joypad::Button;

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct HotkeyConfig {
    pub scale_1: String,
    pub scale_2: String,
    pub scale_3: String,
    pub reset: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            scale_1: "Super+1".into(),
            scale_2: "Super+2".into(),
            scale_3: "Super+3".into(),
            reset: "Ctrl+Super+R".into(),
        }
    }
}

pub struct Hotkey {
    pub key: KeyCode,
    pub ctrl: bool,
    pub super_key: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Hotkey {
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('+').collect();
        let mut ctrl = false;
        let mut super_key = false;
        let mut shift = false;
        let mut alt = false;
        let mut key = None;

        for part in &parts {
            match part.trim() {
                "Ctrl" => ctrl = true,
                "Super" | "Cmd" => super_key = true,
                "Shift" => shift = true,
                "Alt" | "Option" => alt = true,
                k => key = key_name_to_keycode(k),
            }
        }

        key.map(|k| Hotkey {
            key: k,
            ctrl,
            super_key,
            shift,
            alt,
        })
    }

    pub fn matches(&self, key_code: KeyCode, modifiers: &winit::keyboard::ModifiersState) -> bool {
        self.key == key_code
            && self.ctrl == modifiers.control_key()
            && self.super_key == modifiers.super_key()
            && self.shift == modifiers.shift_key()
            && self.alt == modifiers.alt_key()
    }
}

pub struct HotkeyMap {
    pub scale_1: Option<Hotkey>,
    pub scale_2: Option<Hotkey>,
    pub scale_3: Option<Hotkey>,
    pub reset: Option<Hotkey>,
}

impl HotkeyMap {
    pub fn from_config(config: &HotkeyConfig) -> Self {
        Self {
            scale_1: Hotkey::parse(&config.scale_1),
            scale_2: Hotkey::parse(&config.scale_2),
            scale_3: Hotkey::parse(&config.scale_3),
            reset: Hotkey::parse(&config.reset),
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Config {
    pub display: DisplayConfig,
    pub rom: RomConfig,
    pub input: InputConfig,
    pub hotkeys: HotkeyConfig,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub scale: u32,
    pub shader: String,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct RomConfig {
    pub path: String,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct InputConfig {
    pub player1: PlayerInput,
    pub player2: PlayerInput,
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct PlayerInput {
    pub up: String,
    pub down: String,
    pub left: String,
    pub right: String,
    pub a: String,
    pub b: String,
    pub select: String,
    pub start: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            display: DisplayConfig::default(),
            rom: RomConfig::default(),
            input: InputConfig::default(),
            hotkeys: HotkeyConfig::default(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            scale: 3,
            shader: "none".into(),
        }
    }
}

impl Default for RomConfig {
    fn default() -> Self {
        Self {
            path: "./roms".into(),
        }
    }
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            player1: PlayerInput::default_p1(),
            player2: PlayerInput::default_p2(),
        }
    }
}

impl PlayerInput {
    pub fn default_p1() -> Self {
        Self {
            up: "E".into(),
            down: "D".into(),
            left: "S".into(),
            right: "F".into(),
            a: "K".into(),
            b: "J".into(),
            select: "G".into(),
            start: "H".into(),
        }
    }

    pub fn default_p2() -> Self {
        Self {
            up: "Up".into(),
            down: "Down".into(),
            left: "Left".into(),
            right: "Right".into(),
            a: "Numpad2".into(),
            b: "Numpad1".into(),
            select: "Numpad5".into(),
            start: "Numpad6".into(),
        }
    }
}

impl Default for PlayerInput {
    fn default() -> Self {
        Self::default_p1()
    }
}

impl Config {
    pub fn load() -> Self {
        let paths = [
            PathBuf::from("rfc.toml"),
            dirs::config_dir()
                .map(|d| d.join("rfc").join("rfc.toml"))
                .unwrap_or_default(),
        ];

        for path in &paths {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    match toml::from_str(&content) {
                        Ok(config) => {
                            log::info!("Loaded config from {}", path.display());
                            return config;
                        }
                        Err(e) => {
                            log::warn!("Failed to parse {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }

        log::info!("Using default config");
        Config::default()
    }
}

pub fn key_name_to_keycode(name: &str) -> Option<KeyCode> {
    match name {
        // Letters
        "A" => Some(KeyCode::KeyA),
        "B" => Some(KeyCode::KeyB),
        "C" => Some(KeyCode::KeyC),
        "D" => Some(KeyCode::KeyD),
        "E" => Some(KeyCode::KeyE),
        "F" => Some(KeyCode::KeyF),
        "G" => Some(KeyCode::KeyG),
        "H" => Some(KeyCode::KeyH),
        "I" => Some(KeyCode::KeyI),
        "J" => Some(KeyCode::KeyJ),
        "K" => Some(KeyCode::KeyK),
        "L" => Some(KeyCode::KeyL),
        "M" => Some(KeyCode::KeyM),
        "N" => Some(KeyCode::KeyN),
        "O" => Some(KeyCode::KeyO),
        "P" => Some(KeyCode::KeyP),
        "Q" => Some(KeyCode::KeyQ),
        "R" => Some(KeyCode::KeyR),
        "S" => Some(KeyCode::KeyS),
        "T" => Some(KeyCode::KeyT),
        "U" => Some(KeyCode::KeyU),
        "V" => Some(KeyCode::KeyV),
        "W" => Some(KeyCode::KeyW),
        "X" => Some(KeyCode::KeyX),
        "Y" => Some(KeyCode::KeyY),
        "Z" => Some(KeyCode::KeyZ),
        // Digit row
        "1" => Some(KeyCode::Digit1),
        "2" => Some(KeyCode::Digit2),
        "3" => Some(KeyCode::Digit3),
        "4" => Some(KeyCode::Digit4),
        "5" => Some(KeyCode::Digit5),
        "6" => Some(KeyCode::Digit6),
        "7" => Some(KeyCode::Digit7),
        "8" => Some(KeyCode::Digit8),
        "9" => Some(KeyCode::Digit9),
        "0" => Some(KeyCode::Digit0),
        // Arrow keys
        "Up" => Some(KeyCode::ArrowUp),
        "Down" => Some(KeyCode::ArrowDown),
        "Left" => Some(KeyCode::ArrowLeft),
        "Right" => Some(KeyCode::ArrowRight),
        // Special keys
        "Enter" => Some(KeyCode::Enter),
        "Space" => Some(KeyCode::Space),
        "Escape" => Some(KeyCode::Escape),
        "LShift" => Some(KeyCode::ShiftLeft),
        "RShift" => Some(KeyCode::ShiftRight),
        "Tab" => Some(KeyCode::Tab),
        // Numpad
        "Numpad0" => Some(KeyCode::Numpad0),
        "Numpad1" => Some(KeyCode::Numpad1),
        "Numpad2" => Some(KeyCode::Numpad2),
        "Numpad3" => Some(KeyCode::Numpad3),
        "Numpad4" => Some(KeyCode::Numpad4),
        "Numpad5" => Some(KeyCode::Numpad5),
        "Numpad6" => Some(KeyCode::Numpad6),
        "Numpad7" => Some(KeyCode::Numpad7),
        "Numpad8" => Some(KeyCode::Numpad8),
        "Numpad9" => Some(KeyCode::Numpad9),
        _ => None,
    }
}

/// Build a mapping from KeyCode to (Button, player) for quick lookup
pub struct KeyMap {
    pub mappings: Vec<(KeyCode, Button, u8)>, // (key, button, player 1 or 2)
}

impl KeyMap {
    pub fn from_config(config: &InputConfig) -> Self {
        let mut mappings = Vec::new();

        let buttons: &[(&str, Button)] = &[
            ("up", Button::Up),
            ("down", Button::Down),
            ("left", Button::Left),
            ("right", Button::Right),
            ("a", Button::A),
            ("b", Button::B),
            ("select", Button::Select),
            ("start", Button::Start),
        ];

        for &(field, button) in buttons {
            let key_name = match field {
                "up" => &config.player1.up,
                "down" => &config.player1.down,
                "left" => &config.player1.left,
                "right" => &config.player1.right,
                "a" => &config.player1.a,
                "b" => &config.player1.b,
                "select" => &config.player1.select,
                "start" => &config.player1.start,
                _ => continue,
            };
            if let Some(kc) = key_name_to_keycode(key_name) {
                mappings.push((kc, button, 1));
            }
        }

        for &(field, button) in buttons {
            let key_name = match field {
                "up" => &config.player2.up,
                "down" => &config.player2.down,
                "left" => &config.player2.left,
                "right" => &config.player2.right,
                "a" => &config.player2.a,
                "b" => &config.player2.b,
                "select" => &config.player2.select,
                "start" => &config.player2.start,
                _ => continue,
            };
            if let Some(kc) = key_name_to_keycode(key_name) {
                mappings.push((kc, button, 2));
            }
        }

        Self { mappings }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.display.scale, 3);
        assert_eq!(config.input.player1.up, "E");
        assert_eq!(config.input.player1.select, "G");
        assert_eq!(config.input.player1.start, "H");
    }

    #[test]
    fn test_parse_partial_toml() {
        let toml_str = r#"
            [display]
            scale = 4
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.display.scale, 4);
        assert_eq!(config.input.player1.up, "E"); // Default
    }

    #[test]
    fn test_key_name_mapping() {
        assert_eq!(key_name_to_keycode("E"), Some(KeyCode::KeyE));
        assert_eq!(key_name_to_keycode("Up"), Some(KeyCode::ArrowUp));
        assert_eq!(key_name_to_keycode("Numpad5"), Some(KeyCode::Numpad5));
        assert_eq!(key_name_to_keycode("invalid"), None);
    }
}
