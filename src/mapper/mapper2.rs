use super::Mapper;

/// UxROM — switchable 16KB PRG bank + fixed last bank
/// CHR RAM (8KB, writable)
pub struct Mapper2 {
    prg_rom: Vec<u8>,
    chr_ram: Vec<u8>,
    bank_select: u8,
    prg_banks: u8,
}

impl Mapper2 {
    pub fn new(prg_rom: Vec<u8>, chr_rom: Vec<u8>) -> Self {
        let prg_banks = (prg_rom.len() / 16384) as u8;
        let chr_ram = if chr_rom.is_empty() {
            vec![0u8; 8192]
        } else {
            chr_rom
        };
        Self {
            prg_rom,
            chr_ram,
            bank_select: 0,
            prg_banks,
        }
    }
}

impl Mapper for Mapper2 {
    fn cpu_read(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xBFFF => {
                let offset = self.bank_select as usize * 16384 + (addr - 0x8000) as usize;
                self.prg_rom[offset]
            }
            0xC000..=0xFFFF => {
                let offset = (self.prg_banks as usize - 1) * 16384 + (addr - 0xC000) as usize;
                self.prg_rom[offset]
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            self.bank_select = data & 0x0F;
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        if addr <= 0x1FFF {
            self.chr_ram[addr as usize]
        } else {
            0
        }
    }

    fn ppu_write(&mut self, addr: u16, data: u8) {
        if addr <= 0x1FFF {
            self.chr_ram[addr as usize] = data; // CHR RAM is writable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bank_switching() {
        let mut prg = vec![0u8; 16384 * 4];
        prg[0] = 0xAA;
        prg[16384] = 0xBB;
        let mut mapper = Mapper2::new(prg, vec![]);
        assert_eq!(mapper.cpu_read(0x8000), 0xAA);
        mapper.cpu_write(0x8000, 1);
        assert_eq!(mapper.cpu_read(0x8000), 0xBB);
    }

    #[test]
    fn test_fixed_last_bank() {
        let mut prg = vec![0u8; 16384 * 4];
        prg[16384 * 3] = 0xCC;
        let mapper = Mapper2::new(prg, vec![]);
        assert_eq!(mapper.cpu_read(0xC000), 0xCC);
    }

    #[test]
    fn test_chr_ram_writable() {
        let mut mapper = Mapper2::new(vec![0u8; 16384], vec![]);
        mapper.ppu_write(0x0100, 0x55);
        assert_eq!(mapper.ppu_read(0x0100), 0x55);
    }
}
