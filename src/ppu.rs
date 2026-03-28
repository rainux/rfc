use crate::cartridge::Mirroring;
use crate::mapper::Mapper;

const PALETTE_COLORS: [(u8, u8, u8); 64] = [
    (84, 84, 84),
    (0, 30, 116),
    (8, 16, 144),
    (48, 0, 136),
    (68, 0, 100),
    (92, 0, 48),
    (84, 4, 0),
    (60, 24, 0),
    (32, 42, 0),
    (8, 58, 0),
    (0, 64, 0),
    (0, 60, 0),
    (0, 50, 60),
    (0, 0, 0),
    (0, 0, 0),
    (0, 0, 0),
    (152, 150, 152),
    (8, 76, 196),
    (48, 50, 236),
    (92, 30, 228),
    (136, 20, 176),
    (160, 20, 100),
    (152, 34, 32),
    (120, 60, 0),
    (84, 90, 0),
    (40, 114, 0),
    (8, 124, 0),
    (0, 118, 40),
    (0, 102, 120),
    (0, 0, 0),
    (0, 0, 0),
    (0, 0, 0),
    (236, 238, 236),
    (76, 154, 236),
    (120, 124, 236),
    (176, 98, 236),
    (228, 84, 236),
    (236, 88, 180),
    (236, 106, 100),
    (212, 136, 32),
    (160, 170, 0),
    (116, 196, 0),
    (76, 208, 32),
    (56, 204, 108),
    (56, 180, 204),
    (60, 60, 60),
    (0, 0, 0),
    (0, 0, 0),
    (236, 238, 236),
    (168, 204, 236),
    (188, 188, 236),
    (212, 178, 236),
    (236, 174, 236),
    (236, 174, 212),
    (236, 180, 176),
    (228, 196, 144),
    (204, 210, 120),
    (180, 222, 120),
    (168, 226, 144),
    (152, 226, 180),
    (160, 214, 228),
    (160, 162, 160),
    (0, 0, 0),
    (0, 0, 0),
];

pub struct Ppu {
    // VRAM and sprite memory
    pub vram: [u8; 2048],
    pub oam: [u8; 256],
    pub palette: [u8; 32],

    // Registers
    pub ctrl: u8,
    pub mask: u8,
    pub status: u8,
    pub oam_addr: u8,

    // Internal "loopy" registers
    pub v: u16,  // 15-bit current VRAM address
    pub t: u16,  // 15-bit temp VRAM address
    pub x: u8,   // 3-bit fine X scroll
    pub w: bool, // Write toggle

    // Data buffer for $2007 reads
    pub data_buffer: u8,

    // Timing
    pub scanline: u16,
    pub cycle: u16,

    // Output
    pub frame_buffer: Box<[u8; 256 * 240 * 4]>,

    // Flags
    pub nmi_pending: bool,
    pub frame_complete: bool,

    // Mirroring mode
    pub mirroring: Mirroring,

    // Mapper scanline notification (set at cycle 260 of rendering scanlines)
    pub scanline_triggered: bool,
}

impl Ppu {
    pub fn new(mirroring: Mirroring) -> Self {
        Self {
            vram: [0; 2048],
            oam: [0; 256],
            palette: [0; 32],
            ctrl: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            v: 0,
            t: 0,
            x: 0,
            w: false,
            data_buffer: 0,
            scanline: 0,
            cycle: 0,
            frame_buffer: Box::new([0; 256 * 240 * 4]),
            nmi_pending: false,
            frame_complete: false,
            mirroring,
            scanline_triggered: false,
        }
    }

    /// Read from the PPU's internal address space
    fn ppu_read(&self, addr: u16, mapper: &dyn Mapper) -> u8 {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => mapper.ppu_read(addr),
            0x2000..=0x3EFF => {
                let index = self.mirror_nametable_addr(addr);
                self.vram[index]
            }
            0x3F00..=0x3FFF => {
                let index = self.mirror_palette_addr(addr);
                self.palette[index]
            }
            _ => 0,
        }
    }

    /// Write to the PPU's internal address space
    fn ppu_write(&mut self, addr: u16, data: u8, mapper: &mut dyn Mapper) {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => mapper.ppu_write(addr, data),
            0x2000..=0x3EFF => {
                let index = self.mirror_nametable_addr(addr);
                self.vram[index] = data;
            }
            0x3F00..=0x3FFF => {
                let index = self.mirror_palette_addr(addr);
                self.palette[index] = data;
            }
            _ => {}
        }
    }

    /// Convert a nametable address ($2000-$3EFF) to a VRAM index (0-2047)
    fn mirror_nametable_addr(&self, addr: u16) -> usize {
        let addr = (addr - 0x2000) & 0x0FFF; // Strip to 0-FFF range
        let table = addr / 0x0400; // Which nametable (0-3)
        let offset = addr & 0x03FF; // Offset within table

        let base = match self.mirroring {
            Mirroring::Vertical => {
                // Tables 0,2 -> VRAM[0..0x400], tables 1,3 -> VRAM[0x400..0x800]
                match table {
                    0 | 2 => 0x0000,
                    1 | 3 => 0x0400,
                    _ => unreachable!(),
                }
            }
            Mirroring::Horizontal => {
                // Tables 0,1 -> VRAM[0..0x400], tables 2,3 -> VRAM[0x400..0x800]
                match table {
                    0 | 1 => 0x0000,
                    2 | 3 => 0x0400,
                    _ => unreachable!(),
                }
            }
            Mirroring::FourScreen => {
                // Each table maps to its own 1KB region (needs 4KB, but we only have 2KB;
                // four-screen typically uses cartridge RAM — for now just wrap)
                (table as usize) * 0x0400
            }
        };

        (base + offset as usize) & 0x07FF
    }

    /// Convert a palette address ($3F00-$3FFF) to a palette RAM index (0-31)
    fn mirror_palette_addr(&self, addr: u16) -> usize {
        let mut index = (addr & 0x1F) as usize;
        // $3F10/$3F14/$3F18/$3F1C mirror to $3F00/$3F04/$3F08/$3F0C
        if index == 0x10 || index == 0x14 || index == 0x18 || index == 0x1C {
            index -= 0x10;
        }
        index
    }

    /// VRAM address increment amount (1 or 32) based on PPUCTRL bit 2
    fn vram_increment(&self) -> u16 {
        if self.ctrl & 0x04 != 0 { 32 } else { 1 }
    }

    /// Read a PPU register (CPU-facing, $2000-$2007)
    pub fn read_register(&mut self, addr: u16, mapper: &dyn Mapper) -> u8 {
        match addr & 0x0007 {
            // $2002: PPUSTATUS
            2 => {
                let result = (self.status & 0xE0) | (self.data_buffer & 0x1F);
                self.status &= !0x80; // Clear VBlank flag
                self.w = false; // Reset write toggle
                result
            }
            // $2004: OAMDATA
            4 => self.oam[self.oam_addr as usize],
            // $2007: PPUDATA
            7 => {
                let addr = self.v & 0x3FFF;
                let result = if addr >= 0x3F00 {
                    // Palette reads return immediately
                    // But buffer gets filled with nametable data "underneath"
                    self.data_buffer = self.ppu_read(addr - 0x1000, mapper);
                    self.ppu_read(addr, mapper)
                } else {
                    let buffered = self.data_buffer;
                    self.data_buffer = self.ppu_read(addr, mapper);
                    buffered
                };
                self.v = self.v.wrapping_add(self.vram_increment());
                result
            }
            _ => 0,
        }
    }

    /// Write a PPU register (CPU-facing, $2000-$2007)
    pub fn write_register(&mut self, addr: u16, data: u8, mapper: &mut dyn Mapper) {
        match addr & 0x0007 {
            // $2000: PPUCTRL
            0 => {
                let was_nmi_enabled = self.ctrl & 0x80 != 0;
                self.ctrl = data;
                // Update nametable select bits in t
                self.t = (self.t & 0xF3FF) | (((data & 0x03) as u16) << 10);
                // If NMI enable transitions 0->1 while VBlank is set, trigger NMI
                let nmi_enabled = data & 0x80 != 0;
                if !was_nmi_enabled && nmi_enabled && (self.status & 0x80 != 0) {
                    self.nmi_pending = true;
                }
            }
            // $2001: PPUMASK
            1 => {
                self.mask = data;
            }
            // $2003: OAMADDR
            3 => {
                self.oam_addr = data;
            }
            // $2004: OAMDATA
            4 => {
                self.oam[self.oam_addr as usize] = data;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            // $2005: PPUSCROLL
            5 => {
                if !self.w {
                    self.t = (self.t & 0xFFE0) | ((data as u16) >> 3);
                    self.x = data & 0x07;
                    self.w = true;
                } else {
                    self.t = (self.t & 0x8C1F)
                        | (((data & 0x07) as u16) << 12)
                        | (((data & 0xF8) as u16) << 2);
                    self.w = false;
                }
            }
            // $2006: PPUADDR
            6 => {
                if !self.w {
                    self.t = (self.t & 0x00FF) | (((data & 0x3F) as u16) << 8);
                    self.w = true;
                } else {
                    self.t = (self.t & 0xFF00) | (data as u16);
                    self.v = self.t;
                    self.w = false;
                }
            }
            // $2007: PPUDATA
            7 => {
                let vram_addr = self.v & 0x3FFF;
                self.ppu_write(vram_addr, data, mapper);
                self.v = self.v.wrapping_add(self.vram_increment());
            }
            _ => {}
        }
    }

    /// Advance the PPU by one cycle. Handles scanline rendering, scroll updates, and VBlank timing.
    pub fn step(&mut self, mapper: &dyn Mapper) {
        let rendering_enabled = self.mask & 0x18 != 0;

        // Visible scanlines (0-239)
        if self.scanline < 240 {
            if self.cycle == 257 && rendering_enabled {
                self.render_scanline(mapper);
                // Copy horizontal position bits from t to v
                self.v = (self.v & !0x041F) | (self.t & 0x041F);
                // Increment fine/coarse Y for next scanline
                self.increment_y();
            }
            // Notify mapper for scanline counter (IRQ) at cycle 260
            if self.cycle == 260 && rendering_enabled {
                self.scanline_triggered = true;
            }
        }

        // VBlank start at scanline 241, cycle 1
        if self.scanline == 241 && self.cycle == 1 {
            self.status |= 0x80;
            if self.ctrl & 0x80 != 0 {
                self.nmi_pending = true;
            }
        }

        // Pre-render scanline (261): also clock scanline counter
        if self.scanline == 261 && self.cycle == 260 && rendering_enabled {
            self.scanline_triggered = true;
        }

        // Pre-render scanline (261)
        if self.scanline == 261 {
            if self.cycle == 1 {
                // Clear VBlank, sprite 0 hit, sprite overflow
                self.status &= !0xE0;
                self.nmi_pending = false;
            }
            if self.cycle == 257 && rendering_enabled {
                // Copy horizontal position bits from t to v (same as visible scanlines)
                self.v = (self.v & !0x041F) | (self.t & 0x041F);
            }
            // Copy vertical bits from t to v during cycles 280-304
            if self.cycle >= 280 && self.cycle <= 304 && rendering_enabled {
                self.v = (self.v & !0x7BE0) | (self.t & 0x7BE0);
            }
        }

        // Advance cycle/scanline counters
        self.cycle += 1;
        if self.cycle > 340 {
            self.cycle = 0;
            self.scanline += 1;
            if self.scanline > 261 {
                self.scanline = 0;
                self.frame_complete = true;
            }
        }
    }

    /// Render one scanline: background + sprites composited together.
    fn render_scanline(&mut self, mapper: &dyn Mapper) {
        let y = self.scanline as usize;
        if y >= 240 {
            return;
        }

        // 1. Render background pixels into a temp array
        // Each entry is the full palette RAM index (palette_num * 4 + color_index) for opaque,
        // or 0 for transparent (color_index bits 0-1 == 0).
        let mut bg_pixels = [0u8; 256];
        self.render_background_line(mapper, &mut bg_pixels);

        // 2. Evaluate and render sprites
        // (color_index 1-3, palette 0-3, behind_bg, is_sprite0)
        let mut sprite_pixels: [(u8, u8, bool, bool); 256] = [(0, 0, false, false); 256];
        if self.mask & 0x10 != 0 {
            self.render_sprites_line(mapper, &mut sprite_pixels);
        }

        // 3. Composite
        for x in 0..256 {
            let bg = bg_pixels[x];
            let (sp_color, sp_palette, sp_behind, sp_is_sprite0) = sprite_pixels[x];

            let bg_opaque = bg & 0x03 != 0;
            let sp_opaque = sp_color != 0;

            // Sprite-0 hit detection
            if sp_is_sprite0 && bg_opaque && sp_opaque && x < 255 {
                if self.mask & 0x18 == 0x18 {
                    // Both bg and sprites enabled
                    if x >= 8 || (self.mask & 0x06 == 0x06) {
                        // Left column check
                        self.status |= 0x40;
                    }
                }
            }

            // Determine final pixel
            let palette_index = if !bg_opaque && !sp_opaque {
                self.palette[0]
            } else if !bg_opaque && sp_opaque {
                self.palette[0x10 + sp_palette as usize * 4 + sp_color as usize]
            } else if bg_opaque && !sp_opaque {
                self.palette[bg as usize]
            } else {
                // Both opaque - priority decides
                if sp_behind {
                    self.palette[bg as usize]
                } else {
                    self.palette[0x10 + sp_palette as usize * 4 + sp_color as usize]
                }
            };

            self.set_pixel(x, y, palette_index);
        }
    }

    /// Render one scanline of background tiles into a pixel buffer.
    /// Each entry is (palette_num * 4 + color_index) for opaque pixels, 0 for transparent.
    fn render_background_line(&self, mapper: &dyn Mapper, pixels: &mut [u8; 256]) {
        let bg_enabled = self.mask & 0x08 != 0;

        if !bg_enabled {
            // All pixels are transparent background
            return;
        }

        let pattern_base: u16 = if self.ctrl & 0x10 != 0 {
            0x1000
        } else {
            0x0000
        };

        // Extract scroll state from v register
        let fine_y = (self.v >> 12) & 7;
        let mut coarse_x = self.v & 0x001F;
        let coarse_y = (self.v >> 5) & 0x001F;
        let mut nametable = (self.v >> 10) & 0x03;
        let fine_x = self.x;

        // Fetch the first tile
        let mut tile_x_counter = fine_x;
        let nt_addr = 0x2000 | (nametable << 10) | (coarse_y << 5) | coarse_x;
        let tile_id = self.ppu_read(nt_addr, mapper);
        let pattern_addr = pattern_base + (tile_id as u16) * 16 + fine_y;
        let mut pattern_lo = self.ppu_read(pattern_addr, mapper);
        let mut pattern_hi = self.ppu_read(pattern_addr + 8, mapper);

        let attr_addr = 0x23C0 | (nametable << 10) | ((coarse_y / 4) << 3) | (coarse_x / 4);
        let attr_byte = self.ppu_read(attr_addr, mapper);
        let palette_shift = ((coarse_y & 2) << 1) | (coarse_x & 2);
        let mut palette_num = (attr_byte >> palette_shift) & 0x03;

        for pixel_x in 0..256u16 {
            if pixel_x < 8 && self.mask & 0x02 == 0 {
                // Left 8 pixels hidden: leave as 0 (transparent)
            } else {
                let bit = 7 - tile_x_counter;
                let color_lo = (pattern_lo >> bit) & 1;
                let color_hi = (pattern_hi >> bit) & 1;
                let color_index = color_lo | (color_hi << 1);

                if color_index != 0 {
                    pixels[pixel_x as usize] = palette_num * 4 + color_index;
                }
                // color_index == 0 means transparent, pixels already 0
            }

            tile_x_counter += 1;
            if tile_x_counter >= 8 {
                tile_x_counter = 0;
                coarse_x = (coarse_x + 1) & 0x1F;
                if coarse_x == 0 {
                    nametable ^= 0x01;
                }
                let nt_addr = 0x2000 | (nametable << 10) | (coarse_y << 5) | coarse_x;
                let tile_id = self.ppu_read(nt_addr, mapper);
                let pattern_addr = pattern_base + (tile_id as u16) * 16 + fine_y;
                pattern_lo = self.ppu_read(pattern_addr, mapper);
                pattern_hi = self.ppu_read(pattern_addr + 8, mapper);

                let attr_addr = 0x23C0 | (nametable << 10) | ((coarse_y / 4) << 3) | (coarse_x / 4);
                let attr_byte = self.ppu_read(attr_addr, mapper);
                let palette_shift = ((coarse_y & 2) << 1) | (coarse_x & 2);
                palette_num = (attr_byte >> palette_shift) & 0x03;
            }
        }
    }

    /// Evaluate and render sprites for the current scanline.
    fn render_sprites_line(
        &mut self,
        mapper: &dyn Mapper,
        pixels: &mut [(u8, u8, bool, bool); 256],
    ) {
        let sprite_height: u16 = if self.ctrl & 0x20 != 0 { 16 } else { 8 };
        let pattern_base: u16 = if self.ctrl & 0x08 != 0 {
            0x1000
        } else {
            0x0000
        };
        let y = self.scanline as u16;

        let mut sprite_count = 0;

        for i in 0..64usize {
            let sprite_y = self.oam[i * 4] as u16 + 1; // +1: sprites delayed 1 scanline
            if y < sprite_y || y >= sprite_y + sprite_height {
                continue;
            }
            if sprite_count >= 8 {
                self.status |= 0x20; // Sprite overflow (simplified)
                break;
            }
            sprite_count += 1;

            let tile_index = self.oam[i * 4 + 1] as u16;
            let attributes = self.oam[i * 4 + 2];
            let sprite_x = self.oam[i * 4 + 3] as usize;

            let palette = attributes & 0x03;
            let behind_bg = attributes & 0x20 != 0;
            let flip_h = attributes & 0x40 != 0;
            let flip_v = attributes & 0x80 != 0;

            let mut row = y - sprite_y;
            if flip_v {
                row = sprite_height - 1 - row;
            }

            // Get pattern address
            let pattern_addr = if sprite_height == 16 {
                // 8x16 mode: bit 0 of tile index selects pattern table
                let table = (tile_index & 1) * 0x1000;
                let tile = tile_index & 0xFE;
                if row < 8 {
                    table + tile * 16 + row
                } else {
                    table + (tile + 1) * 16 + (row - 8)
                }
            } else {
                pattern_base + tile_index * 16 + row
            };

            let pattern_lo = self.ppu_read(pattern_addr, mapper);
            let pattern_hi = self.ppu_read(pattern_addr + 8, mapper);

            for col in 0..8u8 {
                let px = if flip_h {
                    sprite_x + col as usize
                } else {
                    sprite_x + (7 - col) as usize
                };
                if px >= 256 {
                    continue;
                }

                // Skip if leftmost 8 pixels and mask says hide sprites there
                if px < 8 && self.mask & 0x04 == 0 {
                    continue;
                }

                let bit_lo = (pattern_lo >> col) & 1;
                let bit_hi = (pattern_hi >> col) & 1;
                let color = bit_lo | (bit_hi << 1);

                if color == 0 {
                    continue;
                } // Transparent

                // Only draw if no higher-priority sprite already here
                if pixels[px].0 == 0 {
                    pixels[px] = (color, palette, behind_bg, i == 0);
                }
            }
        }
    }

    /// Write an RGB pixel to the frame buffer using a NES palette index.
    fn set_pixel(&mut self, x: usize, y: usize, palette_index: u8) {
        let (r, g, b) = PALETTE_COLORS[(palette_index & 0x3F) as usize];
        let offset = (y * 256 + x) * 4;
        self.frame_buffer[offset] = r;
        self.frame_buffer[offset + 1] = g;
        self.frame_buffer[offset + 2] = b;
        self.frame_buffer[offset + 3] = 255;
    }

    /// Increment the fine/coarse Y scroll in the v register.
    fn increment_y(&mut self) {
        if (self.v & 0x7000) != 0x7000 {
            self.v += 0x1000; // Increment fine Y
        } else {
            self.v &= !0x7000; // Fine Y = 0
            let mut coarse_y = (self.v & 0x03E0) >> 5;
            if coarse_y == 29 {
                coarse_y = 0;
                self.v ^= 0x0800; // Switch vertical nametable
            } else if coarse_y == 31 {
                coarse_y = 0; // Don't switch nametable
            } else {
                coarse_y += 1;
            }
            self.v = (self.v & !0x03E0) | (coarse_y << 5);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapper::mapper0::Mapper0;

    fn make_mapper() -> Box<Mapper0> {
        Box::new(Mapper0::new(vec![0u8; 16384], vec![0u8; 8192]))
    }

    #[test]
    fn test_ppuctrl_write_and_nametable_bits() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        let mut mapper = make_mapper();
        ppu.write_register(0x2000, 0x03, mapper.as_mut());
        assert_eq!(ppu.ctrl, 0x03);
        assert_eq!(ppu.t & 0x0C00, 0x0C00);
    }

    #[test]
    fn test_ppustatus_clears_vblank_and_resets_latch() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        let mapper = make_mapper();
        ppu.status = 0x80;
        ppu.w = true;
        ppu.data_buffer = 0x1F;
        let val = ppu.read_register(0x2002, mapper.as_ref());
        assert_eq!(val, 0x80 | 0x1F);
        assert_eq!(ppu.status & 0x80, 0); // VBlank cleared
        assert!(!ppu.w); // Latch reset
    }

    #[test]
    fn test_ppuaddr_double_write_sets_v() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        let mut mapper = make_mapper();
        // Write $21 then $08 -> v = $2108
        ppu.write_register(0x2006, 0x21, mapper.as_mut());
        assert!(ppu.w); // After first write, toggle is true
        ppu.write_register(0x2006, 0x08, mapper.as_mut());
        assert!(!ppu.w);
        assert_eq!(ppu.v, 0x2108);
    }

    #[test]
    fn test_ppudata_buffered_read() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        let mapper = make_mapper();

        // Write some data to nametable VRAM
        ppu.vram[0] = 0xAA; // nametable addr $2000 -> vram[0]
        ppu.vram[1] = 0xBB; // nametable addr $2001 -> vram[1]

        // Set v to $2000 (nametable start)
        ppu.v = 0x2000;
        ppu.data_buffer = 0x00;

        // First read returns buffer (stale), fills buffer with vram[0]
        let val = ppu.read_register(0x2007, mapper.as_ref());
        assert_eq!(val, 0x00); // Old buffer
        // v should have incremented to $2001
        assert_eq!(ppu.v, 0x2001);

        // Second read returns $AA (previously buffered), buffers $BB
        let val = ppu.read_register(0x2007, mapper.as_ref());
        assert_eq!(val, 0xAA);
    }

    #[test]
    fn test_ppudata_palette_read_immediate() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        let mapper = make_mapper();

        ppu.palette[0] = 0x30; // BG color
        ppu.v = 0x3F00;

        let val = ppu.read_register(0x2007, mapper.as_ref());
        assert_eq!(val, 0x30); // Palette reads are immediate
    }

    #[test]
    fn test_nametable_mirroring_vertical() {
        let mut ppu = Ppu::new(Mirroring::Vertical);
        let mut mapper = make_mapper();

        // Write to nametable 0 ($2000)
        ppu.v = 0x2000;
        ppu.write_register(0x2007, 0xAA, mapper.as_mut());

        // Should appear at nametable 2 ($2800) due to vertical mirroring
        assert_eq!(ppu.vram[0], 0xAA);
        // Read from $2800
        ppu.v = 0x2800;
        ppu.data_buffer = 0;
        let _ = ppu.read_register(0x2007, mapper.as_ref()); // Fills buffer
        ppu.v = 0x2800;
        ppu.data_buffer = 0;
        // Direct check: nametable 2 maps to same VRAM as nametable 0
        let idx0 = ppu.mirror_nametable_addr(0x2000);
        let idx2 = ppu.mirror_nametable_addr(0x2800);
        assert_eq!(idx0, idx2);
    }

    #[test]
    fn test_nametable_mirroring_horizontal() {
        let ppu = Ppu::new(Mirroring::Horizontal);

        // Horizontal: tables 0,1 share, tables 2,3 share
        let idx0 = ppu.mirror_nametable_addr(0x2000);
        let idx1 = ppu.mirror_nametable_addr(0x2400);
        let idx2 = ppu.mirror_nametable_addr(0x2800);
        let idx3 = ppu.mirror_nametable_addr(0x2C00);

        assert_eq!(idx0, idx1); // 0 and 1 share
        assert_eq!(idx2, idx3); // 2 and 3 share
        assert_ne!(idx0, idx2); // But different from each other
    }

    #[test]
    fn test_palette_mirroring() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        let mut mapper = make_mapper();

        // Write to $3F00 (BG color)
        ppu.v = 0x3F00;
        ppu.write_register(0x2007, 0x0F, mapper.as_mut());

        // $3F10 should mirror to $3F00
        assert_eq!(ppu.palette[0], 0x0F);
        let idx = ppu.mirror_palette_addr(0x3F10);
        assert_eq!(idx, 0);
    }

    #[test]
    fn test_nmi_on_ctrl_enable_during_vblank() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        let mut mapper = make_mapper();

        // VBlank is set, NMI is disabled
        ppu.status = 0x80;
        ppu.ctrl = 0x00;

        // Enable NMI -> should trigger
        ppu.write_register(0x2000, 0x80, mapper.as_mut());
        assert!(ppu.nmi_pending);
    }

    #[test]
    fn test_ppuscroll_double_write() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        let mut mapper = make_mapper();

        ppu.write_register(0x2005, 0b01101_101, mapper.as_mut()); // coarse X = 13, fine X = 5
        assert_eq!(ppu.x, 5);
        assert_eq!(ppu.t & 0x1F, 13);
        assert!(ppu.w);

        ppu.write_register(0x2005, 0b11010_011, mapper.as_mut()); // fine Y = 3, coarse Y = 26
        assert!(!ppu.w);
    }

    #[test]
    fn test_oamdata_write_increments_addr() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        let mut mapper = make_mapper();

        ppu.write_register(0x2003, 0x00, mapper.as_mut()); // OAMADDR = 0
        ppu.write_register(0x2004, 0xAA, mapper.as_mut()); // Write OAM
        assert_eq!(ppu.oam[0], 0xAA);
        assert_eq!(ppu.oam_addr, 1);
    }

    #[test]
    fn test_vram_increment_32() {
        let mut ppu = Ppu::new(Mirroring::Horizontal);
        let mut mapper = make_mapper();

        // Set PPUCTRL bit 2 for +32 increment
        ppu.write_register(0x2000, 0x04, mapper.as_mut());
        ppu.v = 0x2000;
        ppu.write_register(0x2007, 0x42, mapper.as_mut());
        assert_eq!(ppu.v, 0x2020); // Incremented by 32
    }
}
