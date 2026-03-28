use std::collections::VecDeque;
use std::fs;
use std::sync::{Arc, Mutex};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rfc::bus::Bus;
use rfc::cartridge::Cartridge;
use rfc::config::{Config, KeyMap};
use rfc::console::Console;
use rfc::renderer::Renderer;

const NES_WIDTH: u32 = 256;
const NES_HEIGHT: u32 = 240;

struct App {
    console: Console,
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
    scale: u32,
    key_map: KeyMap,
    _audio_stream: Option<cpal::Stream>,
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
        let renderer = Renderer::new(Arc::clone(&window), self.scale);
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
                    renderer.resize(physical_size);
                }
            }
            WindowEvent::RedrawRequested => {
                self.console.step_frame();
                if let Some(renderer) = self.renderer.as_ref() {
                    renderer.render(self.console.frame_buffer());
                }
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let winit::keyboard::PhysicalKey::Code(key_code) = event.physical_key {
                    let pressed = event.state == winit::event::ElementState::Pressed;
                    for &(kc, button, player) in &self.key_map.mappings {
                        if kc == key_code {
                            match player {
                                1 => self.console.bus.joypad1.set_button(button, pressed),
                                2 => self.console.bus.joypad2.set_button(button, pressed),
                                _ => {}
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

    let rom_path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: rfc <rom_file>");
        std::process::exit(1);
    });
    let rom_data = fs::read(&rom_path).expect("Failed to read ROM");
    let cartridge = Cartridge::from_ines(&rom_data).expect("Failed to parse ROM");

    let mut bus = Bus::new();
    bus.load_cartridge(cartridge);
    let mut console = Console::new(bus);
    console.reset();

    // Set up audio output
    let sample_buffer = console.bus.apu.sample_buffer.clone();
    let audio_stream = setup_audio(sample_buffer);

    let event_loop = EventLoop::new().unwrap();
    let scale = config.display.scale;
    let key_map = KeyMap::from_config(&config.input);

    let mut app = App {
        console,
        renderer: None,
        window: None,
        scale,
        key_map,
        _audio_stream: audio_stream,
    };

    event_loop.run_app(&mut app).unwrap();
}

fn setup_audio(sample_buffer: Arc<Mutex<VecDeque<f32>>>) -> Option<cpal::Stream> {
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

    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut buf = sample_buffer.lock().unwrap();
                for sample in data.iter_mut() {
                    *sample = buf.pop_front().unwrap_or(0.0);
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
