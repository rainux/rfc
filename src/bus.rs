pub struct Bus {
    ram: [u8; 2048],
}

impl Bus {
    pub fn new() -> Self {
        Self {
            ram: [0; 2048],
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            // 2KB internal RAM, mirrored every 2KB up to $1FFF
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            // PPU registers (stub)
            0x2000..=0x3FFF => 0,
            // APU + I/O (stub)
            0x4000..=0x4017 => 0,
            // Cartridge space (stub)
            0x4020..=0xFFFF => 0,
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize] = data,
            0x2000..=0x3FFF => {} // PPU stub
            0x4000..=0x4017 => {} // APU/IO stub
            0x4020..=0xFFFF => {} // Cartridge stub
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
}
