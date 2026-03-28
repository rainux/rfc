# rfc — Rainux's FC / Rust FC

A NES (Famicom) emulator written in Rust, built as an experiment to test Claude Code's fully autonomous coding capabilities.

## The Experiment

The initial v0.1.0 was created in ~2 hours with [Claude Code](https://claude.ai/code). The first hour was spent in collaborative discussion — understanding NES hardware concepts, making architectural decisions, and agreeing on a detailed implementation plan. The second hour was fully autonomous execution: Claude implemented all 19 tasks across 8 milestones, compiling, testing, and committing at each step with zero human intervention. The v0.1.0 result was a playable NES emulator with Super Mario Bros. and Contra running.

Subsequent features (Mapper 1/3/4, egui game browser, display shaders, turbo buttons, configurable hotkeys) were added through iterative human-AI pair programming, bringing game library coverage to ~85%.

### Documents

- [Full chat log](chatlogs/chatlog-full.md) — the complete conversation from brainstorming to finished emulator
- [Design spec](docs/superpowers/specs/2026-03-28-nes-emulator-design.md) — architecture, components, and milestones
- [Implementation plan](docs/superpowers/plans/2026-03-28-nes-emulator.md) — 19 tasks with step-by-step instructions

## Features

- **CPU**: Full 6502 instruction set (151 official opcodes), verified against nestest (5003 lines pass)
- **PPU**: Scanline-based rendering with background tiles, sprites, scrolling, and sprite-0 hit detection
- **APU**: Pulse, triangle, and noise channels with lock-free audio output via cpal
- **Mappers**: 0 (NROM), 1 (MMC1), 2 (UxROM), 3 (CNROM), 4 (MMC3) — covers ~85% of NES games
- **Input**: Configurable keyboard mapping with turbo (auto-fire) A/B buttons
- **Display shaders**: None (pixel-perfect), CRT (scanlines + vignette), Smooth (bilinear), Scanline (pure)
- **Game selection**: egui-based ROM browser with directory navigation and CJK filename support
- **ROM formats**: Direct `.nes` files and `.zip` archives (auto-extracts first `.nes` inside)
- **Rendering**: Pure Rust graphics stack (winit + wgpu), aspect ratio locked to 16:15
- **Configurable**: All settings via `rfc.toml` — keys, hotkeys, display scale, shaders

## Usage

```
cargo run                     # Launch with game selection menu
cargo run -- <rom_file.nes>   # Launch a specific ROM directly
```

### Game Selection Menu

On startup (without a ROM argument), a game browser scans the configured ROM directory:

- **Up/Down** or **E/D** — Navigate
- **Left/Right** or **S/F** — Page up/down
- **Enter**, **Start**, **B**, **Select** — Open directory or launch game
- **Backspace** or **A** — Go back to parent directory
- **Esc** — Quit (with confirmation)

### In-Game Controls (Player 1)

| NES | Keyboard |
|-----|----------|
| D-pad | E / S / D / F |
| A | K |
| B | J |
| Turbo A | I |
| Turbo B | U |
| Select | G |
| Start | H |

### Shortcuts

| Action | Key |
|--------|-----|
| Scale 2x | Cmd+1 |
| Scale 4x | Cmd+2 |
| Scale 6x | Cmd+3 |
| Reset console | Ctrl+Cmd+R |
| Exit to menu | Esc (with confirmation, Y/N) |

All shortcuts are configurable via `rfc.toml`. See [`rfc.sample.toml`](rfc.sample.toml) for the full reference.

### Configuration

Copy `rfc.sample.toml` to `rfc.toml` and edit. Config is loaded from:
1. `./rfc.toml`
2. `~/.config/rfc/rfc.toml`
3. Built-in defaults

## Architecture

Bus-centric design mirroring real NES hardware topology:

```
Console (clock coordination)
├── CPU (6502) ◄──► Bus
│                    ├── RAM (2KB)
│                    ├── PPU (2C02)
│                    ├── APU (with lock-free audio buffer)
│                    ├── Cartridge + Mapper
│                    └── Joypad ×2
├── Renderer (winit + wgpu + egui)
└── Config (serde + toml)
```

## Building

```
cargo build --release
```

Requires Rust edition 2024. No external system dependencies — pure Rust stack.

## License

MIT
