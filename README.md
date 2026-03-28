# rfc — Rainux's FC / Rust FC

A NES (Famicom) emulator written in Rust, built as an experiment to test Claude Code's fully autonomous coding capabilities.

## The Experiment

This project was created in a single ~2 hour session with [Claude Code](https://claude.ai/code). The first hour was spent in collaborative discussion — understanding NES hardware concepts, making architectural decisions, and agreeing on a detailed implementation plan. The second hour was fully autonomous execution: Claude implemented all 19 tasks across 8 milestones, compiling, testing, and committing at each step with zero human intervention.

The result is a working NES emulator with instruction-level accuracy. Basic graphics rendering, audio output, and controller input all work correctly. The only minor issue is occasional audio glitches.

### Documents

- [Full chat log](chatlogs/chatlog-full.md) — the complete conversation from brainstorming to finished emulator
- [Design spec](docs/superpowers/specs/2026-03-28-nes-emulator-design.md) — architecture, components, and milestones
- [Implementation plan](docs/superpowers/plans/2026-03-28-nes-emulator.md) — 19 tasks with step-by-step instructions

## Features

- **CPU**: Full 6502 instruction set (151 official opcodes), verified against nestest (5003 lines pass)
- **PPU**: Scanline-based rendering with background tiles, sprites, scrolling, and sprite-0 hit detection
- **APU**: Pulse, triangle, and noise channels with audio output via cpal
- **Mappers**: Mapper 0 (NROM) and Mapper 2 (UxROM)
- **Input**: Configurable keyboard mapping via TOML config file
- **Rendering**: Pure Rust graphics stack (winit + wgpu), no SDL dependency

## Tested Games

- Super Mario Bros. (Mapper 0)
- Contra (Mapper 2)

## Usage

```
cargo run -- <rom_file.nes>
```

### Default Controls (Player 1)

| NES | Keyboard |
|-----|----------|
| D-pad | E / S / D / F |
| A | K |
| B | J |
| Select | G |
| Start | H |

### Shortcuts

| Action | Key |
|--------|-----|
| Scale 2x | Cmd+1 |
| Scale 4x | Cmd+2 |
| Scale 6x | Cmd+3 |
| Reset | Ctrl+Cmd+R |

### Configuration

Create `rfc.toml` in the project directory to customize settings:

```toml
[display]
scale = 3

[rom]
path = "./roms"

[input.player1]
up = "E"
down = "D"
left = "S"
right = "F"
a = "K"
b = "J"
select = "G"
start = "H"
```

## Architecture

Bus-centric design mirroring real NES hardware topology:

```
Console (clock coordination)
├── CPU (6502) ◄──► Bus
│                    ├── RAM (2KB)
│                    ├── PPU (2C02)
│                    ├── APU
│                    ├── Cartridge + Mapper
│                    └── Joypad ×2
```

## Building

```
cargo build --release
```

Requires Rust edition 2024. No external system dependencies — pure Rust stack.

## License

MIT
