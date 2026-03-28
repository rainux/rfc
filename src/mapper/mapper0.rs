use super::Mapper;

/// NROM — no bank switching
/// 16KB PRG: mirrored at $8000 and $C000
/// 32KB PRG: $8000-$FFFF direct
/// 8KB CHR ROM at PPU $0000-$1FFF
pub struct Mapper0 {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
}

impl Mapper0 {
    pub fn new(prg_rom: Vec<u8>, chr_rom: Vec<u8>) -> Self {
        Self { prg_rom, chr_rom }
    }
}

impl Mapper for Mapper0 {
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

    fn cpu_write(&mut self, _addr: u16, _data: u8) {}

    fn ppu_read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => {
                if self.chr_rom.is_empty() {
                    0
                } else {
                    self.chr_rom[addr as usize]
                }
            }
            _ => 0,
        }
    }

    fn ppu_write(&mut self, _addr: u16, _data: u8) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mapper0_32kb_prg() {
        let prg = vec![0u8; 32768];
        let chr = vec![0u8; 8192];
        let mut mapper = Mapper0::new(prg, chr);
        assert_eq!(mapper.cpu_read(0x8000), 0);
        mapper.cpu_write(0x8000, 0xFF);
        assert_eq!(mapper.cpu_read(0x8000), 0);
    }

    #[test]
    fn test_mapper0_16kb_mirror() {
        let mut prg = vec![0u8; 16384];
        prg[0] = 0xAA;
        let chr = vec![0u8; 8192];
        let mapper = Mapper0::new(prg, chr);
        assert_eq!(mapper.cpu_read(0x8000), 0xAA);
        assert_eq!(mapper.cpu_read(0xC000), 0xAA);
    }

    #[test]
    fn test_mapper0_chr_read() {
        let prg = vec![0u8; 16384];
        let mut chr = vec![0u8; 8192];
        chr[0x0100] = 0x55;
        let mapper = Mapper0::new(prg, chr);
        assert_eq!(mapper.ppu_read(0x0100), 0x55);
    }
}
