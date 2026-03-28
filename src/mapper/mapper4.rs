use super::Mapper;
use crate::cartridge::Mirroring;

/// MMC3 — used by SMB2/3, Kirby's Adventure, Mega Man 3-6, etc.
/// Features: PRG/CHR bank switching, scanline IRQ counter, PRG RAM
pub struct Mapper4 {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    prg_ram: Vec<u8>,

    // Bank select register ($8000)
    bank_select: u8,     // bits 0-2: target register
    prg_bank_mode: bool, // bit 6: PRG bank mode
    chr_inversion: bool, // bit 7: CHR A12 inversion

    // Bank registers R0-R7
    bank_registers: [u8; 8],

    // Mirroring ($A000)
    pub mirroring: Mirroring,

    // IRQ
    irq_latch: u8,
    irq_counter: u8,
    irq_reload: bool,
    irq_enabled: bool,
    irq_pending: bool,

    // PRG geometry
    prg_banks: usize, // number of 8KB PRG banks
    chr_banks: usize, // number of 1KB CHR banks
}

impl Mapper4 {
    pub fn new(prg_rom: Vec<u8>, chr_rom: Vec<u8>, mirroring: Mirroring) -> Self {
        let prg_banks = prg_rom.len() / 8192;
        let chr_size = chr_rom.len();
        let chr_banks = if chr_size > 0 { chr_size / 1024 } else { 0 };
        let chr = if chr_rom.is_empty() {
            vec![0u8; 8192] // CHR RAM (8KB)
        } else {
            chr_rom
        };
        Self {
            prg_rom,
            chr_rom: chr,
            prg_ram: vec![0u8; 8192],
            bank_select: 0,
            prg_bank_mode: false,
            chr_inversion: false,
            bank_registers: [0; 8],
            mirroring,
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enabled: false,
            irq_pending: false,
            prg_banks,
            chr_banks,
        }
    }

    fn prg_bank_offset(&self, bank: usize) -> usize {
        (bank % self.prg_banks) * 8192
    }

    fn chr_bank_offset(&self, bank: usize, size: usize) -> usize {
        if self.chr_banks > 0 {
            let bank_1k = bank % self.chr_banks;
            bank_1k * 1024
        } else {
            // CHR RAM: just use the bank value directly
            bank * size
        }
    }
}

impl Mapper for Mapper4 {
    fn cpu_read(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => self.prg_ram[(addr - 0x6000) as usize],
            0x8000..=0x9FFF => {
                let bank = if !self.prg_bank_mode {
                    self.bank_registers[6] as usize
                } else {
                    self.prg_banks - 2
                };
                let offset = self.prg_bank_offset(bank) + (addr - 0x8000) as usize;
                self.prg_rom[offset]
            }
            0xA000..=0xBFFF => {
                let bank = self.bank_registers[7] as usize;
                let offset = self.prg_bank_offset(bank) + (addr - 0xA000) as usize;
                self.prg_rom[offset]
            }
            0xC000..=0xDFFF => {
                let bank = if !self.prg_bank_mode {
                    self.prg_banks - 2
                } else {
                    self.bank_registers[6] as usize
                };
                let offset = self.prg_bank_offset(bank) + (addr - 0xC000) as usize;
                self.prg_rom[offset]
            }
            0xE000..=0xFFFF => {
                let bank = self.prg_banks - 1;
                let offset = self.prg_bank_offset(bank) + (addr - 0xE000) as usize;
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
            0x8000..=0x9FFF => {
                if addr & 1 == 0 {
                    // $8000: Bank select
                    self.bank_select = data & 0x07;
                    self.prg_bank_mode = data & 0x40 != 0;
                    self.chr_inversion = data & 0x80 != 0;
                } else {
                    // $8001: Bank data
                    let reg = self.bank_select as usize;
                    self.bank_registers[reg] = data;
                }
            }
            0xA000..=0xBFFF => {
                if addr & 1 == 0 {
                    // $A000: Mirroring
                    if self.mirroring != Mirroring::FourScreen {
                        self.mirroring = if data & 1 == 0 {
                            Mirroring::Vertical
                        } else {
                            Mirroring::Horizontal
                        };
                    }
                }
                // $A001: PRG RAM protect — we ignore and always allow access
            }
            0xC000..=0xDFFF => {
                if addr & 1 == 0 {
                    // $C000: IRQ latch
                    self.irq_latch = data;
                } else {
                    // $C001: IRQ reload
                    self.irq_reload = true;
                }
            }
            0xE000..=0xFFFF => {
                if addr & 1 == 0 {
                    // $E000: IRQ disable + acknowledge
                    self.irq_enabled = false;
                    self.irq_pending = false;
                } else {
                    // $E001: IRQ enable
                    self.irq_enabled = true;
                }
            }
            _ => {}
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        if addr > 0x1FFF {
            return 0;
        }

        // CHR RAM: direct mapping, no bank switching
        if self.chr_banks == 0 {
            return self.chr_rom[addr as usize & 0x1FFF];
        }

        let bank = if !self.chr_inversion {
            match addr {
                0x0000..=0x03FF => {
                    self.chr_bank_offset(self.bank_registers[0] as usize & 0xFE, 1024)
                        + (addr as usize & 0x3FF)
                }
                0x0400..=0x07FF => {
                    self.chr_bank_offset((self.bank_registers[0] as usize & 0xFE) + 1, 1024)
                        + (addr as usize & 0x3FF)
                }
                0x0800..=0x0BFF => {
                    self.chr_bank_offset(self.bank_registers[1] as usize & 0xFE, 1024)
                        + (addr as usize & 0x3FF)
                }
                0x0C00..=0x0FFF => {
                    self.chr_bank_offset((self.bank_registers[1] as usize & 0xFE) + 1, 1024)
                        + (addr as usize & 0x3FF)
                }
                0x1000..=0x13FF => {
                    self.chr_bank_offset(self.bank_registers[2] as usize, 1024)
                        + (addr as usize & 0x3FF)
                }
                0x1400..=0x17FF => {
                    self.chr_bank_offset(self.bank_registers[3] as usize, 1024)
                        + (addr as usize & 0x3FF)
                }
                0x1800..=0x1BFF => {
                    self.chr_bank_offset(self.bank_registers[4] as usize, 1024)
                        + (addr as usize & 0x3FF)
                }
                0x1C00..=0x1FFF => {
                    self.chr_bank_offset(self.bank_registers[5] as usize, 1024)
                        + (addr as usize & 0x3FF)
                }
                _ => return 0,
            }
        } else {
            match addr {
                0x0000..=0x03FF => {
                    self.chr_bank_offset(self.bank_registers[2] as usize, 1024)
                        + (addr as usize & 0x3FF)
                }
                0x0400..=0x07FF => {
                    self.chr_bank_offset(self.bank_registers[3] as usize, 1024)
                        + (addr as usize & 0x3FF)
                }
                0x0800..=0x0BFF => {
                    self.chr_bank_offset(self.bank_registers[4] as usize, 1024)
                        + (addr as usize & 0x3FF)
                }
                0x0C00..=0x0FFF => {
                    self.chr_bank_offset(self.bank_registers[5] as usize, 1024)
                        + (addr as usize & 0x3FF)
                }
                0x1000..=0x13FF => {
                    self.chr_bank_offset(self.bank_registers[0] as usize & 0xFE, 1024)
                        + (addr as usize & 0x3FF)
                }
                0x1400..=0x17FF => {
                    self.chr_bank_offset((self.bank_registers[0] as usize & 0xFE) + 1, 1024)
                        + (addr as usize & 0x3FF)
                }
                0x1800..=0x1BFF => {
                    self.chr_bank_offset(self.bank_registers[1] as usize & 0xFE, 1024)
                        + (addr as usize & 0x3FF)
                }
                0x1C00..=0x1FFF => {
                    self.chr_bank_offset((self.bank_registers[1] as usize & 0xFE) + 1, 1024)
                        + (addr as usize & 0x3FF)
                }
                _ => return 0,
            }
        };

        if bank < self.chr_rom.len() {
            self.chr_rom[bank]
        } else {
            0
        }
    }

    fn ppu_write(&mut self, addr: u16, data: u8) {
        if addr > 0x1FFF {
            return;
        }

        // Only writable if CHR RAM
        if self.chr_banks == 0 {
            self.chr_rom[addr as usize & 0x1FFF] = data;
            return;
        }

        // CHR ROM is read-only — ignore writes
    }

    fn notify_scanline(&mut self) {
        if self.irq_counter == 0 || self.irq_reload {
            self.irq_counter = self.irq_latch;
            self.irq_reload = false;
        } else {
            self.irq_counter -= 1;
        }

        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_pending = true;
        }
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending
    }

    fn irq_acknowledge(&mut self) {
        self.irq_pending = false;
    }

    fn mirroring(&self) -> Option<Mirroring> {
        Some(self.mirroring)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mapper4(prg_banks_8k: usize, chr_banks_1k: usize) -> Mapper4 {
        let mut prg = vec![0u8; prg_banks_8k * 8192];
        // Write bank number into first byte of each 8KB bank
        for i in 0..prg_banks_8k {
            prg[i * 8192] = i as u8;
        }
        let mut chr = vec![0u8; chr_banks_1k * 1024];
        // Write bank number into first byte of each 1KB bank
        for i in 0..chr_banks_1k {
            chr[i * 1024] = i as u8;
        }
        Mapper4::new(prg, chr, Mirroring::Vertical)
    }

    #[test]
    fn test_prg_bank_mode_0() {
        // Mode 0: $8000=R6, $A000=R7, $C000=second-to-last, $E000=last
        let mut mapper = make_mapper4(16, 8); // 128KB PRG, 8KB CHR

        // Set R6=2, R7=5
        mapper.cpu_write(0x8000, 6); // select R6
        mapper.cpu_write(0x8001, 2); // R6=2
        mapper.cpu_write(0x8000, 7); // select R7
        mapper.cpu_write(0x8001, 5); // R7=5

        assert_eq!(mapper.cpu_read(0x8000), 2); // bank 2
        assert_eq!(mapper.cpu_read(0xA000), 5); // bank 5
        assert_eq!(mapper.cpu_read(0xC000), 14); // second-to-last (16-2=14)
        assert_eq!(mapper.cpu_read(0xE000), 15); // last (16-1=15)
    }

    #[test]
    fn test_prg_bank_mode_1() {
        // Mode 1: $8000=second-to-last, $A000=R7, $C000=R6, $E000=last
        let mut mapper = make_mapper4(16, 8);

        // Set mode 1 + R6=2, R7=5
        mapper.cpu_write(0x8000, 0x46); // bit 6 set, select R6
        mapper.cpu_write(0x8001, 2);
        mapper.cpu_write(0x8000, 0x47); // bit 6 set, select R7
        mapper.cpu_write(0x8001, 5);

        assert_eq!(mapper.cpu_read(0x8000), 14); // second-to-last
        assert_eq!(mapper.cpu_read(0xA000), 5); // bank 5
        assert_eq!(mapper.cpu_read(0xC000), 2); // bank 2 (R6 swapped here)
        assert_eq!(mapper.cpu_read(0xE000), 15); // last
    }

    #[test]
    fn test_chr_bank_normal() {
        // CHR mode 0 (no inversion): R0/R1 at $0000-$0FFF (2KB each), R2-R5 at $1000-$1FFF (1KB each)
        let mut mapper = make_mapper4(16, 32); // 32 x 1KB CHR banks

        mapper.cpu_write(0x8000, 0); // select R0
        mapper.cpu_write(0x8001, 4); // R0=4 (2KB, low bit ignored -> banks 4,5)
        mapper.cpu_write(0x8000, 1); // select R1
        mapper.cpu_write(0x8001, 10); // R1=10 (2KB -> banks 10,11)
        mapper.cpu_write(0x8000, 2); // select R2
        mapper.cpu_write(0x8001, 20); // R2=20
        mapper.cpu_write(0x8000, 3); // select R3
        mapper.cpu_write(0x8001, 21); // R3=21
        mapper.cpu_write(0x8000, 4); // select R4
        mapper.cpu_write(0x8001, 22); // R4=22
        mapper.cpu_write(0x8000, 5); // select R5
        mapper.cpu_write(0x8001, 23); // R5=23

        assert_eq!(mapper.ppu_read(0x0000), 4); // R0 low (bank 4)
        assert_eq!(mapper.ppu_read(0x0400), 5); // R0 high (bank 5)
        assert_eq!(mapper.ppu_read(0x0800), 10); // R1 low (bank 10)
        assert_eq!(mapper.ppu_read(0x0C00), 11); // R1 high (bank 11)
        assert_eq!(mapper.ppu_read(0x1000), 20); // R2
        assert_eq!(mapper.ppu_read(0x1400), 21); // R3
        assert_eq!(mapper.ppu_read(0x1800), 22); // R4
        assert_eq!(mapper.ppu_read(0x1C00), 23); // R5
    }

    #[test]
    fn test_chr_bank_inverted() {
        // CHR mode 1 (inversion): R2-R5 at $0000-$0FFF, R0/R1 at $1000-$1FFF
        let mut mapper = make_mapper4(16, 32);

        mapper.cpu_write(0x8000, 0x80); // bit 7 set (CHR inversion), select R0
        mapper.cpu_write(0x8001, 4);
        mapper.cpu_write(0x8000, 0x81); // select R1
        mapper.cpu_write(0x8001, 10);
        mapper.cpu_write(0x8000, 0x82); // select R2
        mapper.cpu_write(0x8001, 20);
        mapper.cpu_write(0x8000, 0x83); // select R3
        mapper.cpu_write(0x8001, 21);
        mapper.cpu_write(0x8000, 0x84); // select R4
        mapper.cpu_write(0x8001, 22);
        mapper.cpu_write(0x8000, 0x85); // select R5
        mapper.cpu_write(0x8001, 23);

        // Inverted: R2-R5 at $0000, R0-R1 at $1000
        assert_eq!(mapper.ppu_read(0x0000), 20); // R2
        assert_eq!(mapper.ppu_read(0x0400), 21); // R3
        assert_eq!(mapper.ppu_read(0x0800), 22); // R4
        assert_eq!(mapper.ppu_read(0x0C00), 23); // R5
        assert_eq!(mapper.ppu_read(0x1000), 4); // R0 low
        assert_eq!(mapper.ppu_read(0x1400), 5); // R0 high
        assert_eq!(mapper.ppu_read(0x1800), 10); // R1 low
        assert_eq!(mapper.ppu_read(0x1C00), 11); // R1 high
    }

    #[test]
    fn test_irq_counter_countdown() {
        let mut mapper = make_mapper4(16, 8);

        // Set IRQ latch to 3
        mapper.cpu_write(0xC000, 3); // latch = 3
        mapper.cpu_write(0xC001, 0); // reload flag
        mapper.cpu_write(0xE001, 0); // enable IRQ

        // First notify reloads counter from latch (counter was 0)
        mapper.notify_scanline();
        assert!(!mapper.irq_pending()); // counter just reloaded to 3

        // Count down: 3 -> 2
        mapper.notify_scanline();
        assert!(!mapper.irq_pending());

        // 2 -> 1
        mapper.notify_scanline();
        assert!(!mapper.irq_pending());

        // 1 -> 0: IRQ fires
        mapper.notify_scanline();
        assert!(mapper.irq_pending());

        // Acknowledge
        mapper.irq_acknowledge();
        assert!(!mapper.irq_pending());
    }

    #[test]
    fn test_irq_disabled() {
        let mut mapper = make_mapper4(16, 8);

        mapper.cpu_write(0xC000, 1); // latch = 1
        mapper.cpu_write(0xC001, 0); // reload
        mapper.cpu_write(0xE000, 0); // disable IRQ

        mapper.notify_scanline(); // reload
        mapper.notify_scanline(); // 1 -> 0, but IRQ disabled

        assert!(!mapper.irq_pending());
    }

    #[test]
    fn test_prg_ram() {
        let mut mapper = make_mapper4(16, 8);

        mapper.cpu_write(0x6000, 0xAB);
        assert_eq!(mapper.cpu_read(0x6000), 0xAB);
        mapper.cpu_write(0x7FFF, 0xCD);
        assert_eq!(mapper.cpu_read(0x7FFF), 0xCD);
    }

    #[test]
    fn test_mirroring_control() {
        let mut mapper = make_mapper4(16, 8);
        assert_eq!(mapper.mirroring, Mirroring::Vertical);

        mapper.cpu_write(0xA000, 1); // horizontal
        assert_eq!(mapper.mirroring, Mirroring::Horizontal);

        mapper.cpu_write(0xA000, 0); // vertical
        assert_eq!(mapper.mirroring, Mirroring::Vertical);
    }
}
