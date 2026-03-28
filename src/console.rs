use crate::bus::Bus;
use crate::cpu::Cpu;

pub struct Console {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl Console {
    pub fn new(bus: Bus) -> Self {
        Self {
            cpu: Cpu::new(),
            bus,
        }
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
    }

    /// Run until one frame is complete
    pub fn step_frame(&mut self) {
        self.bus.clear_frame_complete();
        loop {
            // Handle DMA cycles
            if self.bus.dma_cycles > 0 {
                let dma = self.bus.dma_cycles;
                self.bus.dma_cycles = 0;
                // Advance PPU for DMA cycles
                for _ in 0..dma * 3 {
                    self.bus.step_ppu();
                }
                self.cpu.cycles += dma as u64;
            }

            let cpu_cycles = self.cpu.step(&mut self.bus) as u16;
            let ppu_cycles = cpu_cycles * 3;
            for _ in 0..ppu_cycles {
                self.bus.step_ppu();
            }

            if self.bus.poll_nmi() {
                self.cpu.nmi(&mut self.bus);
            }

            if self.bus.frame_complete() {
                break;
            }
        }
    }

    pub fn frame_buffer(&self) -> &[u8] {
        self.bus.frame_buffer()
    }
}
