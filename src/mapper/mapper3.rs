use super::Mapper;

/// CNROM — CHR bank switching
pub struct Mapper3 {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    chr_bank: u8,
}

impl Mapper3 {
    pub fn new(prg_rom: Vec<u8>, chr_rom: Vec<u8>) -> Self {
        Self {
            prg_rom,
            chr_rom,
            chr_bank: 0,
        }
    }
}

impl Mapper for Mapper3 {
    fn cpu_read(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xFFFF => {
                let mut index = (addr - 0x8000) as usize;
                if self.prg_rom.len() == 16384 {
                    index %= 16384;
                }
                self.prg_rom[index]
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            self.chr_bank = data & 0x03;
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        if addr <= 0x1FFF {
            let offset = self.chr_bank as usize * 8192 + addr as usize;
            if offset < self.chr_rom.len() {
                self.chr_rom[offset]
            } else {
                0
            }
        } else {
            0
        }
    }

    fn ppu_write(&mut self, _addr: u16, _data: u8) {
        // CHR ROM is read-only
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chr_bank_switching() {
        let mut chr = vec![0u8; 8192 * 4];
        chr[0] = 0xAA;
        chr[8192] = 0xBB;
        chr[8192 * 2] = 0xCC;
        let mut mapper = Mapper3::new(vec![0u8; 32768], chr);
        assert_eq!(mapper.ppu_read(0x0000), 0xAA);
        mapper.cpu_write(0x8000, 1);
        assert_eq!(mapper.ppu_read(0x0000), 0xBB);
        mapper.cpu_write(0x8000, 2);
        assert_eq!(mapper.ppu_read(0x0000), 0xCC);
    }

    #[test]
    fn test_prg_mirroring_16kb() {
        let mut prg = vec![0u8; 16384];
        prg[0] = 0xDD;
        let mapper = Mapper3::new(prg, vec![0u8; 8192]);
        assert_eq!(mapper.cpu_read(0x8000), 0xDD);
        assert_eq!(mapper.cpu_read(0xC000), 0xDD);
    }
}
