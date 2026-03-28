use crate::mapper::Mapper;
use crate::mapper::mapper0::Mapper0;
use crate::mapper::mapper1::Mapper1;
use crate::mapper::mapper2::Mapper2;
use crate::mapper::mapper4::Mapper4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    FourScreen,
}

pub struct Cartridge {
    pub mapper: Box<dyn Mapper>,
    pub mirroring: Mirroring,
}

impl Cartridge {
    pub fn from_ines(data: &[u8]) -> Result<Self, String> {
        if data.len() < 16 || &data[0..4] != b"NES\x1a" {
            return Err("Invalid iNES header".into());
        }

        let prg_rom_size = data[4] as usize * 16384;
        let chr_rom_size = data[5] as usize * 8192;
        let flags6 = data[6];
        let flags7 = data[7];

        let mapper_number = (flags7 & 0xF0) | (flags6 >> 4);

        let mirroring = if flags6 & 0x08 != 0 {
            Mirroring::FourScreen
        } else if flags6 & 0x01 != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };

        let has_trainer = flags6 & 0x04 != 0;
        let prg_start = 16 + if has_trainer { 512 } else { 0 };
        let chr_start = prg_start + prg_rom_size;

        if data.len() < chr_start + chr_rom_size {
            return Err("File too small for declared ROM sizes".into());
        }

        let prg_rom = data[prg_start..prg_start + prg_rom_size].to_vec();
        let chr_rom = if chr_rom_size > 0 {
            data[chr_start..chr_start + chr_rom_size].to_vec()
        } else {
            vec![0u8; 8192] // CHR RAM
        };

        let mapper: Box<dyn Mapper> = match mapper_number {
            0 => Box::new(Mapper0::new(prg_rom, chr_rom)),
            1 => Box::new(Mapper1::new(prg_rom, chr_rom)),
            2 => Box::new(Mapper2::new(prg_rom, chr_rom)),
            3 => Box::new(crate::mapper::mapper3::Mapper3::new(prg_rom, chr_rom)),
            4 => Box::new(Mapper4::new(prg_rom, chr_rom, mirroring)),
            _ => return Err(format!("Unsupported mapper: {}", mapper_number)),
        };

        Ok(Cartridge { mapper, mirroring })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ines_header(prg_banks: u8, chr_banks: u8, flags6: u8, flags7: u8) -> Vec<u8> {
        let mut header = vec![0x4E, 0x45, 0x53, 0x1A];
        header.push(prg_banks);
        header.push(chr_banks);
        header.push(flags6);
        header.push(flags7);
        header.extend_from_slice(&[0u8; 8]);
        header.extend(vec![0u8; prg_banks as usize * 16384]);
        header.extend(vec![0u8; chr_banks as usize * 8192]);
        header
    }

    #[test]
    fn test_parse_valid_ines() {
        let data = make_ines_header(2, 1, 0x01, 0x00);
        let cart = Cartridge::from_ines(&data).unwrap();
        assert_eq!(cart.mirroring, Mirroring::Vertical);
    }

    #[test]
    fn test_horizontal_mirroring() {
        let data = make_ines_header(1, 1, 0x00, 0x00);
        let cart = Cartridge::from_ines(&data).unwrap();
        assert_eq!(cart.mirroring, Mirroring::Horizontal);
    }

    #[test]
    fn test_invalid_header() {
        let data = vec![0u8; 16];
        assert!(Cartridge::from_ines(&data).is_err());
    }

    #[test]
    fn test_unsupported_mapper() {
        let data = make_ines_header(1, 1, 0x50, 0x00); // Mapper 5 (MMC5) unsupported
        assert!(Cartridge::from_ines(&data).is_err());
    }

    #[test]
    fn test_chr_ram_when_no_chr_rom() {
        let data = make_ines_header(1, 0, 0x00, 0x00);
        let cart = Cartridge::from_ines(&data).unwrap();
        assert_eq!(cart.mapper.ppu_read(0x0000), 0);
    }
}
