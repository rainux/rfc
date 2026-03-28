use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::KeyCode;
use winit::window::{Window, WindowId};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rfc::bus::Bus;
use rfc::cartridge::Cartridge;
use rfc::config::{ButtonAction, Config, HotkeyMap, KeyMap};
use rfc::console::Console;
use rfc::joypad::Button;
use rfc::menu::{Menu, RomEntry, scan_roms};
use rfc::renderer::Renderer;

const NES_WIDTH: u32 = 256;
const NES_HEIGHT: u32 = 240;

enum EmulatorState {
    Menu(Menu),
    Playing {
        console: Console,
        _audio_stream: Option<cpal::Stream>,
    },
}

struct App {
    state: EmulatorState,
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
    scale: u32,
    shader: String,
    key_map: KeyMap,
    hotkey_map: HotkeyMap,
    modifiers: winit::keyboard::ModifiersState,
    /// Cached ROM list so we can return to the menu without re-scanning.
    rom_list: Vec<RomEntry>,
    turbo_rate: u8,
}

impl App {
    fn launch_rom(&mut self, path: &PathBuf) {
        let rom_data = match fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Failed to read ROM {}: {}", path.display(), e);
                return;
            }
        };
        let cartridge = match Cartridge::from_ines(&rom_data) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to parse ROM {}: {}", path.display(), e);
                return;
            }
        };

        let mut bus = Bus::new();
        bus.load_cartridge(cartridge);
        let mut console = Console::new(bus);
        console.reset();
        console.bus.joypad1.turbo_rate = self.turbo_rate;
        console.bus.joypad2.turbo_rate = self.turbo_rate;

        let audio_buffer = console.bus.apu.audio_buffer.clone();
        let audio_stream = setup_audio(audio_buffer);

        self.state = EmulatorState::Playing {
            console,
            _audio_stream: audio_stream,
        };
    }

    fn return_to_menu(&mut self) {
        let mut menu = Menu::new(self.rom_list.clone());
        // Try to preserve the previously selected index
        if let EmulatorState::Menu(ref old) = self.state {
            menu.selected = old.selected;
        }
        self.state = EmulatorState::Menu(menu);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let size = winit::dpi::PhysicalSize::new(NES_WIDTH * self.scale, NES_HEIGHT * self.scale);
        let attrs = Window::default_attributes()
            .with_title("rfc — NES Emulator")
            .with_inner_size(size)
            .with_min_inner_size(winit::dpi::PhysicalSize::new(NES_WIDTH, NES_HEIGHT));

        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        let renderer = Renderer::new(Arc::clone(&window), self.scale, &self.shader);
        self.window = Some(window);
        self.renderer = Some(renderer);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    // Maintain 16:15 aspect ratio (256:240)
                    let aspect = 256.0 / 240.0_f64;
                    let w = physical_size.width as f64;
                    let h = physical_size.height as f64;

                    let (new_w, new_h) = if w / h > aspect {
                        // Too wide — shrink width to match height
                        ((h * aspect).round() as u32, physical_size.height)
                    } else {
                        // Too tall — shrink height to match width
                        (physical_size.width, (w / aspect).round() as u32)
                    };

                    let constrained = winit::dpi::PhysicalSize::new(new_w, new_h);
                    if constrained != physical_size {
                        if let Some(window) = self.window.as_ref() {
                            let _ = window.request_inner_size(constrained);
                        }
                    }
                    renderer.resize(constrained);
                }
            }
            WindowEvent::RedrawRequested => {
                let frame = match &mut self.state {
                    EmulatorState::Menu(menu) => menu.render(),
                    EmulatorState::Playing { console, .. } => {
                        console.step_frame();
                        console.frame_buffer()
                    }
                };
                if let Some(renderer) = self.renderer.as_ref() {
                    renderer.render(frame);
                }
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let winit::keyboard::PhysicalKey::Code(key_code) = event.physical_key {
                    let pressed = event.state == winit::event::ElementState::Pressed;

                    match &mut self.state {
                        EmulatorState::Menu(menu) => {
                            if pressed {
                                match key_code {
                                    KeyCode::ArrowUp => menu.move_up(),
                                    KeyCode::ArrowDown => menu.move_down(),
                                    KeyCode::Enter => {
                                        if let Some(entry) = menu.selected_rom().cloned() {
                                            self.launch_rom(&entry.path);
                                        }
                                    }
                                    KeyCode::Escape => event_loop.exit(),
                                    _ => {
                                        // Also handle configured joypad keys for navigation
                                        for &(kc, ref action, player) in &self.key_map.mappings {
                                            if kc == key_code && player == 1 {
                                                match action {
                                                    ButtonAction::Normal(Button::Up) => {
                                                        menu.move_up()
                                                    }
                                                    ButtonAction::Normal(Button::Down) => {
                                                        menu.move_down()
                                                    }
                                                    ButtonAction::Normal(Button::Start) => {
                                                        if let Some(entry) =
                                                            menu.selected_rom().cloned()
                                                        {
                                                            self.launch_rom(&entry.path);
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        EmulatorState::Playing { console, .. } => {
                            if pressed {
                                // Escape returns to menu
                                if key_code == KeyCode::Escape {
                                    self.return_to_menu();
                                    return;
                                }

                                // Scale hotkeys
                                let scales = [
                                    (&self.hotkey_map.scale_1, 1u32),
                                    (&self.hotkey_map.scale_2, 2),
                                    (&self.hotkey_map.scale_3, 3),
                                ];
                                for (hotkey, scale) in &scales {
                                    if let Some(hk) = hotkey {
                                        if hk.matches(key_code, &self.modifiers) {
                                            if let Some(window) = self.window.as_ref() {
                                                self.scale = *scale;
                                                let _ = window.request_inner_size(
                                                    winit::dpi::PhysicalSize::new(
                                                        NES_WIDTH * scale,
                                                        NES_HEIGHT * scale,
                                                    ),
                                                );
                                            }
                                            return;
                                        }
                                    }
                                }

                                // Reset hotkey
                                if let Some(ref hk) = self.hotkey_map.reset {
                                    if hk.matches(key_code, &self.modifiers) {
                                        console.reset();
                                        return;
                                    }
                                }
                            }

                            // Joypad input (press and release)
                            for &(kc, ref action, player) in &self.key_map.mappings {
                                if kc == key_code {
                                    let joypad = match player {
                                        1 => &mut console.bus.joypad1,
                                        2 => &mut console.bus.joypad2,
                                        _ => continue,
                                    };
                                    match action {
                                        ButtonAction::Normal(button) => {
                                            joypad.set_button(*button, pressed);
                                        }
                                        ButtonAction::TurboA => joypad.set_turbo_a(pressed),
                                        ButtonAction::TurboB => joypad.set_turbo_b(pressed),
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::init();

    let config = Config::load();

    let rom_arg = std::env::args().nth(1);

    let scale = config.display.scale;
    let shader = config.display.shader.clone();
    let turbo_rate = config.input.turbo_rate.max(1);
    let key_map = KeyMap::from_config(&config.input);
    let hotkey_map = HotkeyMap::from_config(&config.hotkeys);
    let rom_path_config = config.rom.path.clone();

    // Scan ROMs directory for the menu
    let rom_list = scan_roms(&rom_path_config);

    // Determine initial state
    let initial_state = if let Some(ref path) = rom_arg {
        // Direct ROM launch — skip menu
        let rom_data = fs::read(path).expect("Failed to read ROM");
        let cartridge = Cartridge::from_ines(&rom_data).expect("Failed to parse ROM");
        let mut bus = Bus::new();
        bus.load_cartridge(cartridge);
        let mut console = Console::new(bus);
        console.reset();
        console.bus.joypad1.turbo_rate = turbo_rate;
        console.bus.joypad2.turbo_rate = turbo_rate;
        let audio_buffer = console.bus.apu.audio_buffer.clone();
        let audio_stream = setup_audio(audio_buffer);
        EmulatorState::Playing {
            console,
            _audio_stream: audio_stream,
        }
    } else {
        EmulatorState::Menu(Menu::new(rom_list.clone()))
    };

    let event_loop = EventLoop::new().unwrap();

    let mut app = App {
        state: initial_state,
        renderer: None,
        window: None,
        scale,
        shader,
        key_map,
        hotkey_map,
        modifiers: winit::keyboard::ModifiersState::empty(),
        rom_list,
        turbo_rate,
    };

    event_loop.run_app(&mut app).unwrap();
}

fn setup_audio(audio_buffer: Arc<rfc::audio::AudioBuffer>) -> Option<cpal::Stream> {
    let host = cpal::default_host();
    let device = match host.default_output_device() {
        Some(d) => d,
        None => {
            eprintln!("Warning: No audio output device found, running without sound");
            return None;
        }
    };

    let config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(44100),
        buffer_size: cpal::BufferSize::Default,
    };

    let mut last_sample = 0.0f32;
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for sample in data.iter_mut() {
                    if let Some(s) = audio_buffer.pop() {
                        last_sample = s;
                        *sample = s;
                    } else {
                        // Fade to silence to avoid clicks on underrun
                        last_sample *= 0.995;
                        *sample = last_sample;
                    }
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )
        .ok();

    if let Some(ref s) = stream {
        if let Err(e) = s.play() {
            eprintln!("Warning: Failed to start audio stream: {}", e);
            return None;
        }
    }

    stream
}
