use crate::cartridge::Cartridge;

pub struct Bus {
    ram: [u8; 2048],
    pub cartridge: Option<Cartridge>,
}

impl Bus {
    pub fn new() -> Self {
        Self {
            ram: [0; 2048],
            cartridge: None,
        }
    }

    pub fn load_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            // 2KB internal RAM, mirrored every 2KB up to $1FFF
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            // PPU registers (stub)
            0x2000..=0x3FFF => 0,
            // APU + I/O (stub)
            0x4000..=0x4017 => 0,
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
            0x2000..=0x3FFF => {} // PPU stub
            0x4000..=0x4017 => {} // APU/IO stub
            0x4020..=0xFFFF => {
                if let Some(ref mut cart) = self.cartridge {
                    cart.mapper.cpu_write(addr, data);
                }
            }
            _ => {}
        }
    }
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
}
