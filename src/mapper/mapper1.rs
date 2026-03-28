use super::Mapper;
use crate::cartridge::Mirroring;

/// MMC1 (SxROM) — used by Zelda, Metroid, Mega Man 2, Final Fantasy, etc.
/// Features: serial shift register interface, PRG/CHR bank switching, PRG RAM
pub struct Mapper1 {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>, // CHR ROM or CHR RAM
    prg_ram: Vec<u8>, // 8KB PRG RAM
    chr_is_ram: bool,

    // Shift register
    shift_register: u8,
    write_count: u8,

    // Internal registers
    control: u8,    // Mirroring, PRG mode, CHR mode
    chr_bank_0: u8, // CHR bank 0
    chr_bank_1: u8, // CHR bank 1
    prg_bank: u8,   // PRG bank

    prg_banks: usize, // Total 16KB PRG banks
    chr_banks: usize, // Total 4KB CHR banks
}

impl Mapper1 {
    pub fn new(prg_rom: Vec<u8>, chr_rom: Vec<u8>) -> Self {
        let prg_banks = prg_rom.len() / 16384;
        let chr_is_ram = chr_rom.is_empty();
        let chr_banks = if chr_is_ram { 2 } else { chr_rom.len() / 4096 };
        let chr = if chr_is_ram { vec![0u8; 8192] } else { chr_rom };
        Self {
            prg_rom,
            chr_rom: chr,
            prg_ram: vec![0u8; 8192],
            chr_is_ram,
            shift_register: 0,
            write_count: 0,
            control: 0x0C, // PRG mode 3 (fix last bank)
            chr_bank_0: 0,
            chr_bank_1: 0,
            prg_bank: 0,
            prg_banks,
            chr_banks,
        }
    }

    fn write_register(&mut self, addr: u16, value: u8) {
        let register = ((addr >> 13) & 0x03) as u8;
        match register {
            0 => self.control = value,
            1 => self.chr_bank_0 = value,
            2 => self.chr_bank_1 = value,
            3 => self.prg_bank = value & 0x0F,
            _ => unreachable!(),
        }
    }

    fn prg_mode(&self) -> u8 {
        (self.control >> 2) & 0x03
    }

    fn chr_mode(&self) -> bool {
        self.control & 0x10 != 0
    }
}

impl Mapper for Mapper1 {
    fn cpu_read(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => self.prg_ram[(addr - 0x6000) as usize],
            0x8000..=0xBFFF => {
                let bank = match self.prg_mode() {
                    0 | 1 => (self.prg_bank as usize & 0xFE) % self.prg_banks,
                    2 => 0,
                    3 => (self.prg_bank as usize) % self.prg_banks,
                    _ => unreachable!(),
                };
                let offset = bank * 16384 + (addr - 0x8000) as usize;
                self.prg_rom[offset]
            }
            0xC000..=0xFFFF => {
                let bank = match self.prg_mode() {
                    0 | 1 => ((self.prg_bank as usize & 0xFE) + 1) % self.prg_banks,
                    2 => (self.prg_bank as usize) % self.prg_banks,
                    3 => self.prg_banks - 1,
                    _ => unreachable!(),
                };
                let offset = bank * 16384 + (addr - 0xC000) as usize;
                self.prg_rom[offset]
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, data: u8) {
        match addr {
            0x6000..=0x7FFF => {
                self.prg_ram[(addr - 0x6000) as usize] = data;
            }
            0x8000..=0xFFFF => {
                if data & 0x80 != 0 {
                    // Reset shift register, set PRG mode to 3
                    self.shift_register = 0;
                    self.write_count = 0;
                    self.control |= 0x0C;
                } else {
                    self.shift_register |= (data & 1) << self.write_count;
                    self.write_count += 1;
                    if self.write_count == 5 {
                        let value = self.shift_register;
                        self.write_register(addr, value);
                        self.shift_register = 0;
                        self.write_count = 0;
                    }
                }
            }
            _ => {}
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        if addr <= 0x1FFF {
            if self.chr_mode() {
                // 4KB mode
                let bank = if addr < 0x1000 {
                    self.chr_bank_0 as usize % self.chr_banks
                } else {
                    self.chr_bank_1 as usize % self.chr_banks
                };
                let offset_in_bank = (addr & 0x0FFF) as usize;
                let offset = bank * 4096 + offset_in_bank;
                if offset < self.chr_rom.len() {
                    self.chr_rom[offset]
                } else {
                    0
                }
            } else {
                // 8KB mode: bit 0 ignored
                let bank = (self.chr_bank_0 as usize >> 1) % (self.chr_banks / 2).max(1);
                let offset = bank * 8192 + addr as usize;
                if offset < self.chr_rom.len() {
                    self.chr_rom[offset]
                } else {
                    0
                }
            }
        } else {
            0
        }
    }

    fn ppu_write(&mut self, addr: u16, data: u8) {
        if addr <= 0x1FFF && self.chr_is_ram {
            if self.chr_mode() {
                let bank = if addr < 0x1000 {
                    self.chr_bank_0 as usize % self.chr_banks
                } else {
                    self.chr_bank_1 as usize % self.chr_banks
                };
                let offset = bank * 4096 + (addr & 0x0FFF) as usize;
                if offset < self.chr_rom.len() {
                    self.chr_rom[offset] = data;
                }
            } else {
                let bank = (self.chr_bank_0 as usize >> 1) % (self.chr_banks / 2).max(1);
                let offset = bank * 8192 + addr as usize;
                if offset < self.chr_rom.len() {
                    self.chr_rom[offset] = data;
                }
            }
        }
    }

    fn mirroring(&self) -> Option<Mirroring> {
        match self.control & 0x03 {
            0 | 1 => None, // One-screen mirroring not representable, fall back to cartridge default
            2 => Some(Mirroring::Vertical),
            3 => Some(Mirroring::Horizontal),
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: perform 5 serial writes to set a register value
    fn serial_write(mapper: &mut Mapper1, addr: u16, value: u8) {
        for i in 0..5 {
            mapper.cpu_write(addr, (value >> i) & 1);
        }
    }

    #[test]
    fn test_shift_register_accumulates_5_writes() {
        // Write value 0b10101 = 21 to control register ($8000)
        let prg = vec![0u8; 16384 * 4];
        let chr = vec![0u8; 8192];
        let mut mapper = Mapper1::new(prg, chr);

        // Value 0b10101 = bit0=1, bit1=0, bit2=1, bit3=0, bit4=1
        mapper.cpu_write(0x8000, 1); // bit 0 = 1
        mapper.cpu_write(0x8000, 0); // bit 1 = 0
        mapper.cpu_write(0x8000, 1); // bit 2 = 1
        mapper.cpu_write(0x8000, 0); // bit 3 = 0
        mapper.cpu_write(0x8000, 1); // bit 4 = 1
        // Control should now be 0b10101 = 21
        assert_eq!(mapper.control, 21);
    }

    #[test]
    fn test_shift_register_reset_on_bit7() {
        let prg = vec![0u8; 16384 * 4];
        let chr = vec![0u8; 8192];
        let mut mapper = Mapper1::new(prg, chr);

        // Start some writes
        mapper.cpu_write(0x8000, 1);
        mapper.cpu_write(0x8000, 1);
        // Reset with bit 7
        mapper.cpu_write(0x8000, 0x80);
        assert_eq!(mapper.write_count, 0);
        assert_eq!(mapper.shift_register, 0);
        // Control should have bits 2-3 set (PRG mode 3)
        assert!(mapper.control & 0x0C == 0x0C);
    }

    #[test]
    fn test_prg_banking_mode3_fix_last_switch_first() {
        let mut prg = vec![0u8; 16384 * 4]; // 4 banks
        prg[0] = 0xAA; // Bank 0 start
        prg[16384] = 0xBB; // Bank 1 start
        prg[16384 * 2] = 0xCC; // Bank 2 start
        prg[16384 * 3] = 0xDD; // Bank 3 (last) start
        let mut mapper = Mapper1::new(prg, vec![]);

        // Default: mode 3 (fix last at $C000, switch $8000)
        assert_eq!(mapper.cpu_read(0x8000), 0xAA); // Bank 0 at $8000
        assert_eq!(mapper.cpu_read(0xC000), 0xDD); // Last bank at $C000

        // Switch bank 2 to $8000
        serial_write(&mut mapper, 0xE000, 2);
        assert_eq!(mapper.cpu_read(0x8000), 0xCC); // Bank 2 at $8000
        assert_eq!(mapper.cpu_read(0xC000), 0xDD); // Last bank still at $C000
    }

    #[test]
    fn test_prg_banking_mode2_fix_first_switch_last() {
        let mut prg = vec![0u8; 16384 * 4];
        prg[0] = 0xAA;
        prg[16384] = 0xBB;
        prg[16384 * 2] = 0xCC;
        prg[16384 * 3] = 0xDD;
        let mut mapper = Mapper1::new(prg, vec![]);

        // Set control to mode 2: bits 2-3 = 0b10
        // control value: mirroring=0, prg_mode=2, chr_mode=0 → 0b01000 = 8
        serial_write(&mut mapper, 0x8000, 0x08);

        assert_eq!(mapper.cpu_read(0x8000), 0xAA); // First bank fixed
        assert_eq!(mapper.cpu_read(0xC000), 0xAA); // Bank 0 at $C000 (prg_bank=0)

        // Switch bank 2 to $C000
        serial_write(&mut mapper, 0xE000, 2);
        assert_eq!(mapper.cpu_read(0x8000), 0xAA); // First bank still fixed
        assert_eq!(mapper.cpu_read(0xC000), 0xCC); // Bank 2 at $C000
    }

    #[test]
    fn test_chr_4kb_banking() {
        let mut chr = vec![0u8; 4096 * 4]; // 4 x 4KB banks
        chr[0] = 0x11; // Bank 0
        chr[4096] = 0x22; // Bank 1
        chr[4096 * 2] = 0x33; // Bank 2
        chr[4096 * 3] = 0x44; // Bank 3
        let prg = vec![0u8; 16384 * 2];
        let mut mapper = Mapper1::new(prg, chr);

        // Set CHR 4KB mode (control bit 4 = 1): 0b1_11_00 = 0x1C
        serial_write(&mut mapper, 0x8000, 0x1C);

        // Set CHR bank 0 = 1
        serial_write(&mut mapper, 0xA000, 1);
        assert_eq!(mapper.ppu_read(0x0000), 0x22);

        // Set CHR bank 1 = 3
        serial_write(&mut mapper, 0xC000, 3);
        assert_eq!(mapper.ppu_read(0x1000), 0x44);
    }

    #[test]
    fn test_mirroring_changes() {
        let prg = vec![0u8; 16384 * 2];
        let chr = vec![0u8; 8192];
        let mut mapper = Mapper1::new(prg, chr);

        // Set vertical mirroring: control bits 0-1 = 2, rest preserve
        serial_write(&mut mapper, 0x8000, 0x0E); // 0b01110 = mode3 + vertical
        assert_eq!(mapper.mirroring(), Some(Mirroring::Vertical));

        // Set horizontal mirroring: control bits 0-1 = 3
        serial_write(&mut mapper, 0x8000, 0x0F); // 0b01111 = mode3 + horizontal
        assert_eq!(mapper.mirroring(), Some(Mirroring::Horizontal));
    }

    #[test]
    fn test_prg_ram_read_write() {
        let prg = vec![0u8; 16384 * 2];
        let mut mapper = Mapper1::new(prg, vec![]);
        mapper.cpu_write(0x6000, 0x42);
        assert_eq!(mapper.cpu_read(0x6000), 0x42);
        mapper.cpu_write(0x7FFF, 0xFF);
        assert_eq!(mapper.cpu_read(0x7FFF), 0xFF);
    }

    #[test]
    fn test_chr_ram_writable() {
        let prg = vec![0u8; 16384 * 2];
        let mut mapper = Mapper1::new(prg, vec![]); // No CHR ROM = CHR RAM
        mapper.ppu_write(0x0100, 0xAB);
        assert_eq!(mapper.ppu_read(0x0100), 0xAB);
    }
}
