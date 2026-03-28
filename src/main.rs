use std::fs;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use rfc::bus::Bus;
use rfc::cartridge::Cartridge;
use rfc::console::Console;
use rfc::renderer::Renderer;

const NES_WIDTH: u32 = 256;
const NES_HEIGHT: u32 = 240;

struct App {
    console: Console,
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
    scale: u32,
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
            _ => {}
        }
    }
}

fn main() {
    env_logger::init();

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

    let event_loop = EventLoop::new().unwrap();
    let scale = 3u32;

    let mut app = App {
        console,
        renderer: None,
        window: None,
        scale,
    };

    event_loop.run_app(&mut app).unwrap();
}
