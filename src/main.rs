use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
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
use rfc::menu::Menu;
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

/// Pending confirmation dialog
#[derive(Clone, PartialEq)]
enum ConfirmAction {
    QuitApp,
    ReturnToMenu,
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
    rom_path: String,
    turbo_rate: u8,
    confirm_pending: Option<ConfirmAction>,
}

impl App {
    fn launch_rom(&mut self, path: &PathBuf) {
        let rom_data = match load_rom_data(path) {
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

        // Snap window to NES scale size when entering game
        if let Some(window) = self.window.as_ref() {
            let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                NES_WIDTH * self.scale,
                NES_HEIGHT * self.scale,
            ));
        }
    }

    fn render_confirm_dialog(
        ctx: &egui::Context,
        message: &str,
        confirmed: &mut bool,
        cancelled: &mut bool,
    ) {
        egui::Window::new("Confirm")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new(message).size(18.0));
                    ui.add_space(15.0);
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        if ui
                            .button(egui::RichText::new("  Yes  ").size(16.0))
                            .clicked()
                        {
                            *confirmed = true;
                        }
                        ui.add_space(20.0);
                        if ui
                            .button(egui::RichText::new("  No  ").size(16.0))
                            .clicked()
                        {
                            *cancelled = true;
                        }
                    });
                    ui.add_space(10.0);
                });
            });

        // Keyboard shortcuts: Enter/Y = confirm, N/Esc = cancel
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Y) {
                *confirmed = true;
            }
            if i.key_pressed(egui::Key::N) {
                *cancelled = true;
            }
        });
    }

    fn return_to_menu(&mut self) {
        self.state = EmulatorState::Menu(Menu::new_with_path(&self.rom_path));
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let size = winit::dpi::PhysicalSize::new(NES_WIDTH * self.scale, NES_HEIGHT * self.scale);
        let attrs = Window::default_attributes()
            .with_title("rfc \u{2014} NES Emulator")
            .with_inner_size(size)
            .with_min_inner_size(winit::dpi::PhysicalSize::new(NES_WIDTH, NES_HEIGHT));

        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        let renderer = Renderer::new(Arc::clone(&window), self.scale, &self.shader);
        self.window = Some(window);
        self.renderer = Some(renderer);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Let egui handle events when in Menu state or when confirmation dialog is showing
        let egui_active =
            matches!(&self.state, EmulatorState::Menu(_)) || self.confirm_pending.is_some();
        if egui_active {
            if let Some(renderer) = self.renderer.as_mut() {
                if let Some(window) = self.window.as_ref() {
                    let consumed = renderer.handle_event(window, &event);
                    // If egui consumed the event, skip further processing
                    // (but still handle CloseRequested and Resized)
                    match &event {
                        WindowEvent::CloseRequested
                        | WindowEvent::Resized(_)
                        | WindowEvent::RedrawRequested => {}
                        WindowEvent::KeyboardInput { .. } if !consumed => {}
                        _ => {
                            if consumed {
                                return;
                            }
                        }
                    }
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    match &self.state {
                        EmulatorState::Playing { .. } => {
                            // Maintain 16:15 aspect ratio (256:240) only during gameplay
                            let aspect = 256.0 / 240.0_f64;
                            let w = physical_size.width as f64;
                            let h = physical_size.height as f64;

                            let (new_w, new_h) = if w / h > aspect {
                                ((h * aspect).round() as u32, physical_size.height)
                            } else {
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
                        EmulatorState::Menu(_) => {
                            // Allow free resizing for the menu
                            renderer.resize(physical_size);
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                match &mut self.state {
                    EmulatorState::Menu(menu) => {
                        let confirm = self.confirm_pending.clone();
                        if let (Some(renderer), Some(window)) =
                            (self.renderer.as_mut(), self.window.as_ref())
                        {
                            let mut confirmed = false;
                            let mut cancelled = false;
                            renderer.render_egui(window, |ctx| {
                                menu.ui(ctx);
                                if confirm.is_some() {
                                    Self::render_confirm_dialog(
                                        ctx,
                                        "Quit rfc?",
                                        &mut confirmed,
                                        &mut cancelled,
                                    );
                                }
                            });
                            if confirmed {
                                event_loop.exit();
                                return;
                            }
                            if cancelled {
                                self.confirm_pending = None;
                            }
                        }
                        // Check if a ROM should be launched
                        if self.confirm_pending.is_none() {
                            if let EmulatorState::Menu(menu) = &mut self.state {
                                if menu.should_launch {
                                    menu.should_launch = false;
                                    if let Some(path) = menu.launch_path.take() {
                                        self.launch_rom(&path);
                                    }
                                }
                            }
                        }
                    }
                    EmulatorState::Playing { console, .. } => {
                        if self.confirm_pending.is_some() {
                            // Paused — render game frame with egui overlay on top
                            let mut confirmed = false;
                            let mut cancelled = false;
                            let fb = console.frame_buffer();
                            if let (Some(renderer), Some(window)) =
                                (self.renderer.as_mut(), self.window.as_ref())
                            {
                                renderer.render_with_egui_overlay(window, fb, |ctx| {
                                    Self::render_confirm_dialog(
                                        ctx,
                                        "Return to game list?",
                                        &mut confirmed,
                                        &mut cancelled,
                                    );
                                });
                            }
                            if confirmed {
                                self.confirm_pending = None;
                                self.return_to_menu();
                                return;
                            }
                            if cancelled {
                                self.confirm_pending = None;
                            }
                        } else {
                            console.step_frame();
                            if let Some(renderer) = self.renderer.as_ref() {
                                renderer.render(console.frame_buffer());
                            }
                        }
                    }
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

                    // Handle confirm dialog keys globally when active
                    if pressed && self.confirm_pending.is_some() {
                        match key_code {
                            KeyCode::KeyY | KeyCode::Enter => {
                                match self.confirm_pending.take().unwrap() {
                                    ConfirmAction::QuitApp => {
                                        event_loop.exit();
                                        return;
                                    }
                                    ConfirmAction::ReturnToMenu => {
                                        self.return_to_menu();
                                        return;
                                    }
                                }
                            }
                            KeyCode::KeyN | KeyCode::Escape => {
                                self.confirm_pending = None;
                                return;
                            }
                            _ => return, // Ignore other keys during confirm
                        }
                    }

                    match &mut self.state {
                        EmulatorState::Menu(menu) => {
                            if pressed {
                                match key_code {
                                    KeyCode::Escape => {
                                        self.confirm_pending = Some(ConfirmAction::QuitApp);
                                    }
                                    _ => {
                                        // Handle configured joypad keys for navigation
                                        for &(kc, ref action, player) in &self.key_map.mappings {
                                            if kc == key_code && player == 1 {
                                                match action {
                                                    ButtonAction::Normal(Button::Up) => {
                                                        menu.move_up()
                                                    }
                                                    ButtonAction::Normal(Button::Down) => {
                                                        menu.move_down()
                                                    }
                                                    ButtonAction::Normal(Button::Left) => {
                                                        menu.page_up()
                                                    }
                                                    ButtonAction::Normal(Button::Right) => {
                                                        menu.page_down()
                                                    }
                                                    ButtonAction::Normal(Button::Start)
                                                    | ButtonAction::Normal(Button::B)
                                                    | ButtonAction::Normal(Button::Select) => {
                                                        menu.activate_selected();
                                                    }
                                                    ButtonAction::Normal(Button::A) => {
                                                        menu.go_back();
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
                                // Escape triggers confirmation dialog
                                if key_code == KeyCode::Escape {
                                    self.confirm_pending = Some(ConfirmAction::ReturnToMenu);
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

    // Determine initial state
    let initial_state = if let Some(ref path) = rom_arg {
        // Direct ROM launch — skip menu
        let rom_data = load_rom_data(Path::new(path)).expect("Failed to read ROM");
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
        EmulatorState::Menu(Menu::new_with_path(&rom_path_config))
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
        rom_path: rom_path_config,
        turbo_rate,
        confirm_pending: None,
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

/// Load ROM data from a .nes file or extract the first .nes from a .zip archive.
fn load_rom_data(path: &Path) -> Result<Vec<u8>, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "zip" {
        let file = fs::File::open(path).map_err(|e| format!("{}", e))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Invalid zip: {}", e))?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| format!("{}", e))?;
            if entry.name().to_lowercase().ends_with(".nes") {
                let mut data = Vec::new();
                entry.read_to_end(&mut data).map_err(|e| format!("{}", e))?;
                return Ok(data);
            }
        }
        Err("No .nes file found in zip archive".into())
    } else {
        fs::read(path).map_err(|e| format!("{}", e))
    }
}
