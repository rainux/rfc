# RFC (Rainux's FC / Rust FC) — NES Emulator Design Spec

## Overview

A NES (Famicom) emulator written in Rust, targeting instruction-level accuracy. The project uses a pure Rust graphics/audio stack (winit + wgpu + cpal) and a bus-centric architecture that mirrors real hardware topology.

## Goals & Non-Goals

### Goals

- Instruction-level accurate emulation of NES hardware
- Run nestest.nes with full log-level correctness
- Play Super Mario Bros. (Mapper 0), Contra (Mapper 2)
- Pure Rust dependencies — `cargo build` with no system libraries
- Configurable input and display via TOML config file

### Non-Goals (for now)

- Cycle-accurate emulation
- Illegal/unofficial CPU opcodes
- Save states, rewind, or debugging UI
- Network play
- Mapper 4 (MMC3) / Bad Apple (deferred to future work)

## Architecture

### Hardware Topology

```
┌─────────────────────────────────────┐
│              Console                │
│  (clock coordination, per-frame)    │
│                                     │
│  ┌─────┐    ┌───────────────────┐   │
│  │ CPU │◄──►│       Bus         │   │
│  │6502 │    │                   │   │
│  └─────┘    │  ┌─────┐ ┌─────┐ │   │
│             │  │ RAM │ │ PPU │ │   │
│             │  │ 2KB │ │2C02 │ │   │
│             │  └─────┘ └─────┘ │   │
│             │  ┌─────┐ ┌─────┐ │   │
│             │  │ APU │ │Cart │ │   │
│             │  │(stub)│ │+Map │ │   │
│             │  └─────┘ └─────┘ │   │
│             │  ┌──────────┐    │   │
│             │  │Joypad x2 │    │   │
│             │  └──────────┘    │   │
│             └───────────────────┘   │
└─────────────────────────────────────┘
```

### Key Design Decisions

1. **Bus-centric routing**: CPU interacts with all peripherals exclusively through `Bus::read(addr)` / `Bus::write(addr, data)`. CPU holds no references to PPU, APU, or Cartridge.

2. **Address map** (CPU bus):
   - `$0000–$07FF` → 2KB internal RAM (mirrored to `$1FFF`)
   - `$2000–$2007` → PPU registers (mirrored to `$3FFF`)
   - `$4000–$4017` → APU + I/O (joypad)
   - `$4020–$FFFF` → Cartridge (via Mapper)

3. **Ownership chain**: `Console` owns `Cpu` + `Bus`. `Bus` owns `Ppu`, `Apu`, `Cartridge`, `Joypad`, and RAM. No circular references. CPU's `step()` takes `&mut Bus` as a parameter.

4. **Interrupt signaling**: PPU sets an NMI-pending flag on the Bus. After each CPU step, Console checks the flag and invokes `cpu.nmi()` if set. No callbacks or circular references.

5. **Clock synchronization**: Instruction-level. After each CPU instruction (N cycles), PPU advances N×3 PPU cycles. PPU renders per-scanline, not per-pixel.

## Component Details

### CPU (6502)

```rust
pub struct Cpu {
    pub a: u8,          // Accumulator
    pub x: u8,          // Index register X
    pub y: u8,          // Index register Y
    pub sp: u8,         // Stack pointer
    pub pc: u16,        // Program counter
    pub status: u8,     // Status flags: NV-BDIZC
    pub cycles: u64,    // Total cycles elapsed
}
```

- Implements all 56 official instructions across 13 addressing modes (151 valid opcodes).
- `step(&mut self, bus: &mut Bus) -> u8`: execute one instruction, return cycle count.
- Unofficial opcodes: panic on encounter (aids debugging, can be relaxed later).
- Addressing modes: Immediate, ZeroPage, ZeroPage+X, ZeroPage+Y, Absolute, Absolute+X, Absolute+Y, Indirect, Indirect+X, Indirect+Y, Relative, Accumulator, Implied.
- Page-crossing penalty: +1 cycle when indexed addressing crosses a page boundary (for read instructions).

### PPU (2C02)

```rust
pub struct Ppu {
    pub vram: [u8; 2048],           // 2KB nametable memory
    pub oam: [u8; 256],             // Sprite attribute memory (64 sprites × 4 bytes)
    pub palette: [u8; 32],          // Palette RAM
    pub ctrl: u8,                   // $2000 PPUCTRL
    pub mask: u8,                   // $2001 PPUMASK
    pub status: u8,                 // $2002 PPUSTATUS
    pub oam_addr: u8,               // $2003 OAMADDR
    pub scroll: PpuScroll,          // $2005 scroll state (internal t/v/x/w registers)
    pub addr: PpuAddr,              // $2006 address state
    pub data_buffer: u8,            // $2007 read buffer
    pub scanline: u16,              // Current scanline (0–261)
    pub cycle: u16,                 // Current cycle within scanline (0–340)
    pub frame_buffer: [u8; 256 * 240],  // Pixel output (palette indices → RGB in renderer)
    pub nmi_pending: bool,          // NMI interrupt flag
}
```

- Rendering is per-scanline (not per-pixel). Sufficient for instruction-level accuracy.
- Visible scanlines: 0–239. VBlank begins at scanline 241, sets bit 7 of PPUSTATUS and raises NMI (if enabled in PPUCTRL).
- Pre-render scanline (261): clears VBlank and sprite-0 hit flags.
- Background rendering: fetches nametable + attribute + pattern data per tile per scanline.
- Sprite rendering: evaluates OAM per scanline, supports 8 sprites per scanline limit, sprite-0 hit detection.
- Nametable mirroring: horizontal or vertical, determined by cartridge header.

### APU (initial stub)

```rust
pub struct Apu {
    // Registers for pulse1, pulse2, triangle, noise, DMC channels
    // Initial implementation: accept writes, produce no output
}
```

- Accepts register writes at `$4000–$4013`, `$4015`, `$4017`.
- Full implementation deferred to milestone 8.

### Bus

```rust
pub struct Bus {
    pub ram: [u8; 2048],
    pub ppu: Ppu,
    pub apu: Apu,
    pub cartridge: Cartridge,
    pub joypad1: Joypad,
    pub joypad2: Joypad,
}
```

- `read(&mut self, addr: u16) -> u8`: route by address range.
- `write(&mut self, addr: u16, data: u8)`: route by address range.
- Handles address mirroring (RAM mirrors every 2KB, PPU registers mirror every 8 bytes).
- OAM DMA ($4014): copies 256 bytes from CPU memory to PPU OAM, consumes 513/514 cycles.

### Cartridge + Mapper

```rust
pub struct Cartridge {
    pub prg_rom: Vec<u8>,       // Program ROM
    pub chr_rom: Vec<u8>,       // Character ROM (tile graphics)
    pub mapper: Box<dyn Mapper>,
    pub mirroring: Mirroring,   // Horizontal / Vertical / FourScreen
}

pub trait Mapper {
    fn cpu_read(&self, addr: u16) -> u8;
    fn cpu_write(&mut self, addr: u16, data: u8);
    fn ppu_read(&self, addr: u16) -> u8;
    fn ppu_write(&mut self, addr: u16, data: u8);
}
```

- iNES format parser: reads 16-byte header, extracts PRG/CHR ROM sizes, mapper number, mirroring mode.
- **Mapper 0 (NROM)**: Direct mapping. 16KB or 32KB PRG ROM, 8KB CHR ROM. No bank switching.
- **Mapper 2 (UxROM)**: Switchable 16KB PRG bank at `$8000–$BFFF`, fixed last bank at `$C000–$FFFF`. Bank selected by writing to `$8000–$FFFF`. Uses CHR RAM (8KB).

### Joypad

```rust
pub struct Joypad {
    pub strobe: bool,
    pub button_index: u8,
    pub button_state: u8,   // 8 bits: A/B/Select/Start/Up/Down/Left/Right
}
```

- NES polls joypad via `$4016` (player 1) and `$4017` (player 2).
- Write `$4016` bit 0 to toggle strobe. While strobe is high, reads return button A state. When strobe goes low, successive reads return each button in sequence.

### Configuration (`rfc.toml`)

```toml
[display]
scale = 3                # Window scale multiplier (base: 256×240)

[rom]
path = "./roms"          # ROM directory (future: game list on startup)

[input.player1]
up = "E"
down = "D"
left = "S"
right = "F"
a = "K"
b = "J"
select = "G"
start = "H"

[input.player2]
up = "Up"
down = "Down"
left = "Left"
right = "Right"
a = "Numpad2"
b = "Numpad1"
select = "Numpad5"
start = "Numpad6"
```

- Parsed at startup by `config.rs` using `serde` + `toml` crate.
- Search order: `./rfc.toml` → `~/.config/rfc/rfc.toml` → built-in defaults.
- Key names map to `winit::keyboard::KeyCode` variants.

### Renderer (winit + wgpu)

- `winit` creates the window and runs the event loop.
- Each frame: PPU's `frame_buffer` (palette indices) is converted to RGBA, uploaded as a wgpu texture, and rendered as a full-screen quad.
- Window size = 256 × 240 × `scale` from config.
- VSync driven by winit's `RequestRedraw`.

## File Structure

```
src/
├── main.rs          # Entry point: load config, init window, run main loop
├── console.rs       # Console: owns all components, drives per-frame execution
├── cpu.rs           # CPU: 6502 instruction set implementation
├── bus.rs           # Bus: address routing and mirroring
├── ppu.rs           # PPU: graphics rendering
├── apu.rs           # APU: audio (stub initially)
├── cartridge.rs     # iNES parser + Cartridge struct
├── mapper/
│   ├── mod.rs       # Mapper trait definition + factory function
│   ├── mapper0.rs   # NROM
│   └── mapper2.rs   # UxROM
├── joypad.rs        # Joypad input handling
├── config.rs        # TOML config parsing
└── renderer.rs      # winit + wgpu frontend rendering
```

## Milestones

| #   | Milestone                         | Verification                                     | User Action Required                  |
| --- | --------------------------------- | ------------------------------------------------ | ------------------------------------- |
| 1   | CPU + Bus + RAM skeleton          | Compiles, unit tests pass for basic instructions | No                                    |
| 2   | iNES parser + Mapper 0            | Load nestest.nes, verify header parsing          | No (nestest downloaded automatically) |
| 3   | Complete CPU instruction set      | nestest full log comparison passes               | No                                    |
| 4   | PPU basic rendering               | Render SMB title screen                          | Yes (provide SMB ROM)                 |
| 5   | PPU complete: scrolling + sprites | SMB shows full gameplay graphics                 | No                                    |
| 6   | Joypad input + config             | SMB is playable with configured keys             | No                                    |
| 7   | Mapper 2                          | Contra is playable                               | Yes (provide Contra ROM)              |
| 8   | APU                               | Audio output works                               | No                                    |

## Dependencies (Cargo.toml)

```toml
[dependencies]
winit = "0.30"
wgpu = "24"
pollster = "0.4"          # Block on async wgpu calls
serde = { version = "1", features = ["derive"] }
toml = "0.8"
cpal = "0.15"
dirs = "6"                # Resolve ~/.config path
log = "0.4"
env_logger = "0.11"
```

## Implementation Notes

These details surfaced during spec review. Not blocking, but need resolution during implementation:

1. **PPU CHR data access path**: PPU rendering needs to read CHR ROM/RAM from the cartridge. The Mapper trait has `ppu_read`/`ppu_write` for this. During implementation, PPU will access CHR data through a reference to the cartridge (passed into PPU's rendering methods), not through the CPU bus.

2. **OAM DMA cycle accounting**: `Bus::write` to `$4014` triggers a 513/514-cycle DMA transfer. Since `Bus::write` has no return value, the Bus will track pending DMA cycles in a field (e.g., `dma_cycles: u16`), which Console checks after each CPU step to advance the clock accordingly.

3. **CHR RAM writability**: Mapper 2 uses CHR RAM (writable) instead of CHR ROM. The `chr_rom` field in Cartridge will be used for both cases — Mapper's `ppu_write` implementation will allow writes when the cartridge has CHR RAM.

## Future Work (out of scope for initial implementation)

- Mapper 4 (MMC3) — enables Bad Apple and many more games
- APU DMC channel
- Illegal/unofficial CPU opcodes
- Save states and rewind
- Game list UI on startup
- Debugging tools (CPU/PPU state viewer, breakpoints)
- Cycle-accurate emulation
