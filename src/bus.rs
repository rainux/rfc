use crate::apu::Apu;
use crate::cartridge::{Cartridge, Mirroring};
use crate::joypad::Joypad;
use crate::ppu::Ppu;

pub struct Bus {
    ram: [u8; 2048],
    pub ppu: Ppu,
    pub apu: Apu,
    pub cartridge: Option<Cartridge>,
    pub dma_cycles: u16,
    pub joypad1: Joypad,
    pub joypad2: Joypad,
}

impl Bus {
    pub fn new() -> Self {
        Self {
            ram: [0; 2048],
            ppu: Ppu::new(Mirroring::Horizontal),
            apu: Apu::new(),
            cartridge: None,
            dma_cycles: 0,
            joypad1: Joypad::new(),
            joypad2: Joypad::new(),
        }
    }

    pub fn load_cartridge(&mut self, cartridge: Cartridge) {
        self.ppu = Ppu::new(cartridge.mirroring);
        self.cartridge = Some(cartridge);
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            // 2KB internal RAM, mirrored every 2KB up to $1FFF
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            // PPU registers, mirrored every 8 bytes
            0x2000..=0x3FFF => {
                let ppu_addr = 0x2000 + (addr & 0x0007);
                if let Some(ref cart) = self.cartridge {
                    // Safe: we only borrow cart immutably for the mapper ref
                    let mapper: &dyn crate::mapper::Mapper = cart.mapper.as_ref();
                    self.ppu.read_register(ppu_addr, mapper)
                } else {
                    self.ppu.read_register(ppu_addr, &NullMapper)
                }
            }
            // APU + I/O
            0x4000..=0x4014 => 0, // APU write-only registers
            0x4015 => self.apu.read(addr),
            0x4016 => self.joypad1.read(),
            0x4017 => self.joypad2.read(),
            // Cartridge space
            0x4020..=0xFFFF => {
                if let Some(ref cart) = self.cartridge {
                    cart.mapper.cpu_read(addr)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize] = data,
            0x2000..=0x3FFF => {
                let ppu_addr = 0x2000 + (addr & 0x0007);
                if let Some(ref mut cart) = self.cartridge {
                    self.ppu.write_register(ppu_addr, data, cart.mapper.as_mut());
                } else {
                    self.ppu.write_register(ppu_addr, data, &mut NullMapper);
                }
            }
            0x4000..=0x4013 => self.apu.write(addr, data),
            0x4014 => {
                // OAM DMA: copy 256 bytes from CPU page to PPU OAM
                let page = (data as u16) << 8;
                for i in 0..256u16 {
                    let val = self.read(page + i);
                    self.ppu.oam[self.ppu.oam_addr as usize] = val;
                    self.ppu.oam_addr = self.ppu.oam_addr.wrapping_add(1);
                }
                self.dma_cycles = 513;
            }
            0x4015 => self.apu.write(addr, data),
            0x4016 => {
                self.joypad1.write(data);
                self.joypad2.write(data); // Same strobe signal goes to both
            }
            0x4017 => self.apu.write(addr, data), // APU frame counter (write only; reads go to joypad2)
            0x4020..=0xFFFF => {
                if let Some(ref mut cart) = self.cartridge {
                    cart.mapper.cpu_write(addr, data);
                }
            }
            _ => {}
        }
    }

    pub fn step_apu(&mut self) {
        self.apu.step();
    }

    pub fn step_ppu(&mut self) {
        if let Some(ref cart) = self.cartridge {
            self.ppu.step(cart.mapper.as_ref());
        }
    }

    pub fn poll_nmi(&mut self) -> bool {
        let pending = self.ppu.nmi_pending;
        self.ppu.nmi_pending = false;
        pending
    }

    pub fn frame_complete(&self) -> bool {
        self.ppu.frame_complete
    }

    pub fn frame_buffer(&self) -> &[u8] {
        &*self.ppu.frame_buffer
    }

    pub fn clear_frame_complete(&mut self) {
        self.ppu.frame_complete = false;
    }
}

/// A no-op mapper used when no cartridge is loaded
struct NullMapper;

impl crate::mapper::Mapper for NullMapper {
    fn cpu_read(&self, _addr: u16) -> u8 { 0 }
    fn cpu_write(&mut self, _addr: u16, _data: u8) {}
    fn ppu_read(&self, _addr: u16) -> u8 { 0 }
    fn ppu_write(&mut self, _addr: u16, _data: u8) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ram_read_write() {
        let mut bus = Bus::new();
        bus.write(0x0000, 0x42);
        assert_eq!(bus.read(0x0000), 0x42);
    }

    #[test]
    fn test_ram_mirroring() {
        let mut bus = Bus::new();
        bus.write(0x0000, 0xAB);
        assert_eq!(bus.read(0x0800), 0xAB);
        assert_eq!(bus.read(0x1000), 0xAB);
        assert_eq!(bus.read(0x1800), 0xAB);
    }

    #[test]
    fn test_ram_mirror_write() {
        let mut bus = Bus::new();
        bus.write(0x0800, 0xCD);
        assert_eq!(bus.read(0x0000), 0xCD);
    }

    #[test]
    fn test_cartridge_read() {
        let mut bus = Bus::new();
        let mut prg = vec![0u8; 32768];
        prg[0] = 0xEA; // NOP at $8000
        prg[0x7FFC] = 0x00; // Reset vector low ($FFFC)
        prg[0x7FFD] = 0x80; // Reset vector high -> $8000
        let chr = vec![0u8; 8192];
        let cart = crate::cartridge::Cartridge {
            mapper: Box::new(crate::mapper::mapper0::Mapper0::new(prg, chr)),
            mirroring: crate::cartridge::Mirroring::Horizontal,
        };
        bus.load_cartridge(cart);
        assert_eq!(bus.read(0x8000), 0xEA);
        assert_eq!(bus.read(0xFFFC), 0x00);
        assert_eq!(bus.read(0xFFFD), 0x80);
    }

    #[test]
    fn test_ppu_register_mirroring() {
        let mut bus = Bus::new();
        // Writing to $2000 and $2008 should both hit PPUCTRL
        bus.write(0x2000, 0x03);
        assert_eq!(bus.ppu.ctrl, 0x03);
        bus.write(0x2008, 0x01); // Mirror of $2000
        assert_eq!(bus.ppu.ctrl, 0x01);
    }

    #[test]
    fn test_oam_dma() {
        let mut bus = Bus::new();
        // Fill RAM page $02 (addr $0200-$02FF) with test data
        for i in 0..256u16 {
            bus.write(0x0200 + i, i as u8);
        }
        bus.ppu.oam_addr = 0;
        bus.write(0x4014, 0x02); // DMA from page $02
        assert_eq!(bus.ppu.oam[0], 0x00);
        assert_eq!(bus.ppu.oam[1], 0x01);
        assert_eq!(bus.ppu.oam[255], 0xFF);
        assert_eq!(bus.dma_cycles, 513);
    }
}
