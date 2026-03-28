# RFC NES Emulator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a playable NES emulator in Rust with instruction-level accuracy, supporting Mapper 0 and 2.

**Architecture:** Bus-centric design mirroring real hardware. CPU communicates with all peripherals through a central Bus. Console orchestrates clock synchronization at instruction level (PPU runs 3× CPU clock). Pure Rust graphics/audio stack (winit + wgpu + cpal).

**Tech Stack:** Rust (edition 2024), winit, wgpu, cpal, serde, toml

**Spec:** `docs/superpowers/specs/2026-03-28-nes-emulator-design.md`

---

## File Structure

```
src/
├── main.rs          # Entry point: load config, init window, run main loop
├── console.rs       # Console: owns Cpu + Bus, drives per-frame execution
├── cpu.rs           # CPU: 6502 registers, step(), instruction decoding + execution
├── bus.rs           # Bus: owns PPU/APU/Cartridge/Joypad/RAM, address routing
├── ppu.rs           # PPU: rendering state machine, scanline-based rendering
├── apu.rs           # APU: audio registers (stub initially)
├── cartridge.rs     # iNES parser + Cartridge struct
├── mapper/
│   ├── mod.rs       # Mapper trait + factory function
│   ├── mapper0.rs   # NROM
│   └── mapper2.rs   # UxROM
├── joypad.rs        # Joypad: strobe protocol, button state
├── config.rs        # TOML config parsing with defaults
└── renderer.rs      # winit + wgpu: window, texture upload, render loop
```

---

## Milestone 1: CPU + Bus + RAM Skeleton

### Task 1: Project setup and dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add all dependencies to Cargo.toml**

```toml
[package]
name = "rfc"
version = "0.1.0"
edition = "2024"

[dependencies]
winit = "0.30"
wgpu = "24"
pollster = "0.4"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
cpal = "0.15"
dirs = "6"
log = "0.4"
env_logger = "0.11"

[dev-dependencies]
```

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml check`
Expected: compiles successfully

- [ ] **Step 2: Commit**

```
git add Cargo.toml Cargo.lock
git commit -m "Add project dependencies"
```

### Task 2: Bus — address routing skeleton

**Files:**
- Create: `src/bus.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create Bus with RAM and read/write routing**

`src/bus.rs`:
```rust
pub struct Bus {
    ram: [u8; 2048],
}

impl Bus {
    pub fn new() -> Self {
        Self {
            ram: [0; 2048],
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            // 2KB internal RAM, mirrored every 2KB up to $1FFF
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            // PPU registers (stub)
            0x2000..=0x3FFF => 0,
            // APU + I/O (stub)
            0x4000..=0x4017 => 0,
            // Cartridge space (stub)
            0x4020..=0xFFFF => 0,
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize] = data,
            0x2000..=0x3FFF => {} // PPU stub
            0x4000..=0x4017 => {} // APU/IO stub
            0x4020..=0xFFFF => {} // Cartridge stub
            _ => {}
        }
    }
}
```

- [ ] **Step 2: Update main.rs to declare bus module**

`src/main.rs`:
```rust
mod bus;

fn main() {
    println!("Hello, world!");
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml check`
Expected: compiles successfully

- [ ] **Step 4: Write unit tests for Bus RAM read/write and mirroring**

Add to bottom of `src/bus.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ram_read_write() {
        let mut bus = Bus::new();
        bus.write(0x0000, 0x42);
        assert_eq!(bus.read(0x0000), 0x42);
    }

    #[test]
    fn test_ram_mirroring() {
        let mut bus = Bus::new();
        bus.write(0x0000, 0xAB);
        // $0800 mirrors to $0000
        assert_eq!(bus.read(0x0800), 0xAB);
        // $1000 mirrors to $0000
        assert_eq!(bus.read(0x1000), 0xAB);
        // $1800 mirrors to $0000
        assert_eq!(bus.read(0x1800), 0xAB);
    }

    #[test]
    fn test_ram_mirror_write() {
        let mut bus = Bus::new();
        bus.write(0x0800, 0xCD);
        assert_eq!(bus.read(0x0000), 0xCD);
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml test`
Expected: 3 tests pass

- [ ] **Step 6: Commit**

```
git add src/bus.rs src/main.rs
git commit -m "Add Bus with RAM read/write and address mirroring"
```

### Task 3: CPU — registers and basic infrastructure

**Files:**
- Create: `src/cpu.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create CPU struct with registers and reset logic**

`src/cpu.rs`:
```rust
use crate::bus::Bus;

/// Status register flag bits
const CARRY: u8 = 0b0000_0001;
const ZERO: u8 = 0b0000_0010;
const INTERRUPT_DISABLE: u8 = 0b0000_0100;
const DECIMAL: u8 = 0b0000_1000;
const BREAK: u8 = 0b0001_0000;
const UNUSED: u8 = 0b0010_0000;
const OVERFLOW: u8 = 0b0100_0000;
const NEGATIVE: u8 = 0b1000_0000;

pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    pub status: u8,
    pub cycles: u64,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFD,
            pc: 0,
            status: UNUSED | INTERRUPT_DISABLE,
            cycles: 0,
        }
    }

    /// Read the reset vector and set PC
    pub fn reset(&mut self, bus: &mut Bus) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFD;
        self.status = UNUSED | INTERRUPT_DISABLE;
        let lo = bus.read(0xFFFC) as u16;
        let hi = bus.read(0xFFFD) as u16;
        self.pc = (hi << 8) | lo;
        self.cycles = 7; // Reset takes 7 cycles
    }

    // Flag helpers
    fn get_flag(&self, flag: u8) -> bool {
        self.status & flag != 0
    }

    fn set_flag(&mut self, flag: u8, value: bool) {
        if value {
            self.status |= flag;
        } else {
            self.status &= !flag;
        }
    }

    fn update_zero_and_negative(&mut self, value: u8) {
        self.set_flag(ZERO, value == 0);
        self.set_flag(NEGATIVE, value & 0x80 != 0);
    }

    /// Push a byte onto the stack
    fn push(&mut self, bus: &mut Bus, data: u8) {
        bus.write(0x0100 | self.sp as u16, data);
        self.sp = self.sp.wrapping_sub(1);
    }

    /// Pull a byte from the stack
    fn pull(&mut self, bus: &mut Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.read(0x0100 | self.sp as u16)
    }

    /// Push 16-bit value (high byte first, then low byte)
    fn push16(&mut self, bus: &mut Bus, data: u16) {
        self.push(bus, (data >> 8) as u8);
        self.push(bus, data as u8);
    }

    /// Pull 16-bit value
    fn pull16(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.pull(bus) as u16;
        let hi = self.pull(bus) as u16;
        (hi << 8) | lo
    }
}
```

- [ ] **Step 2: Add cpu module to main.rs**

`src/main.rs`:
```rust
mod bus;
mod cpu;

fn main() {
    println!("Hello, world!");
}
```

- [ ] **Step 3: Write tests for CPU initialization and stack operations**

Add to bottom of `src/cpu.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_new() {
        let cpu = Cpu::new();
        assert_eq!(cpu.a, 0);
        assert_eq!(cpu.x, 0);
        assert_eq!(cpu.y, 0);
        assert_eq!(cpu.sp, 0xFD);
        assert_eq!(cpu.pc, 0);
        assert_eq!(cpu.status, UNUSED | INTERRUPT_DISABLE);
    }

    #[test]
    fn test_reset_reads_vector() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();
        // Set reset vector at $FFFC/$FFFD — these are in cartridge space,
        // Bus returns 0 for unmapped reads, so PC will be 0x0000.
        // We'll test proper reset vector reading after cartridge is wired up.
        cpu.reset(&mut bus);
        assert_eq!(cpu.sp, 0xFD);
        assert_eq!(cpu.status, UNUSED | INTERRUPT_DISABLE);
        assert_eq!(cpu.cycles, 7);
    }

    #[test]
    fn test_push_pull() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();
        cpu.sp = 0xFF;
        cpu.push(&mut bus, 0x42);
        assert_eq!(cpu.sp, 0xFE);
        let val = cpu.pull(&mut bus);
        assert_eq!(val, 0x42);
        assert_eq!(cpu.sp, 0xFF);
    }

    #[test]
    fn test_push16_pull16() {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();
        cpu.sp = 0xFF;
        cpu.push16(&mut bus, 0x1234);
        assert_eq!(cpu.sp, 0xFD);
        let val = cpu.pull16(&mut bus);
        assert_eq!(val, 0x1234);
    }

    #[test]
    fn test_flag_operations() {
        let mut cpu = Cpu::new();
        cpu.set_flag(CARRY, true);
        assert!(cpu.get_flag(CARRY));
        cpu.set_flag(CARRY, false);
        assert!(!cpu.get_flag(CARRY));
    }

    #[test]
    fn test_update_zero_and_negative() {
        let mut cpu = Cpu::new();
        cpu.update_zero_and_negative(0);
        assert!(cpu.get_flag(ZERO));
        assert!(!cpu.get_flag(NEGATIVE));

        cpu.update_zero_and_negative(0x80);
        assert!(!cpu.get_flag(ZERO));
        assert!(cpu.get_flag(NEGATIVE));

        cpu.update_zero_and_negative(0x01);
        assert!(!cpu.get_flag(ZERO));
        assert!(!cpu.get_flag(NEGATIVE));
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml test`
Expected: all tests pass

- [ ] **Step 5: Commit**

```
git add src/cpu.rs src/main.rs
git commit -m "Add CPU struct with registers, flags, and stack operations"
```

### Task 4: CPU — addressing modes

**Files:**
- Modify: `src/cpu.rs`

The 6502 has 13 addressing modes. Each mode resolves to an address (or immediate value) that instructions operate on. We implement them as methods that return `(address, page_crossed)`.

- [ ] **Step 1: Add addressing mode enum and resolution methods**

Add to `src/cpu.rs` (before `impl Cpu`):
```rust
#[derive(Debug, Clone, Copy)]
pub enum AddressingMode {
    Implicit,
    Accumulator,
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Relative,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndirectX,
    IndirectY,
}
```

Add to `impl Cpu`:
```rust
    /// Read a byte at PC and advance PC
    fn fetch(&mut self, bus: &mut Bus) -> u8 {
        let data = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        data
    }

    /// Read a 16-bit word at PC (little-endian) and advance PC by 2
    fn fetch16(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.fetch(bus) as u16;
        let hi = self.fetch(bus) as u16;
        (hi << 8) | lo
    }

    /// Read a 16-bit word from addr, handling the 6502 page-boundary bug
    /// (indirect JMP bug: if addr is $xxFF, high byte wraps within the page)
    fn read16_wrap(&self, bus: &mut Bus, addr: u16) -> u16 {
        let lo = bus.read(addr) as u16;
        // Wrap within same page for high byte
        let hi_addr = (addr & 0xFF00) | ((addr.wrapping_add(1)) & 0x00FF);
        let hi = bus.read(hi_addr) as u16;
        (hi << 8) | lo
    }

    /// Resolve addressing mode to (effective_address, page_crossed)
    /// For Immediate mode, returns the address of the operand byte (PC before advance).
    fn resolve_address(&mut self, bus: &mut Bus, mode: AddressingMode) -> (u16, bool) {
        match mode {
            AddressingMode::Implicit | AddressingMode::Accumulator => (0, false),

            AddressingMode::Immediate => {
                let addr = self.pc;
                self.pc = self.pc.wrapping_add(1);
                (addr, false)
            }

            AddressingMode::ZeroPage => {
                let addr = self.fetch(bus) as u16;
                (addr, false)
            }

            AddressingMode::ZeroPageX => {
                let base = self.fetch(bus);
                (base.wrapping_add(self.x) as u16, false)
            }

            AddressingMode::ZeroPageY => {
                let base = self.fetch(bus);
                (base.wrapping_add(self.y) as u16, false)
            }

            AddressingMode::Relative => {
                let offset = self.fetch(bus) as i8;
                let addr = self.pc.wrapping_add(offset as u16);
                (addr, false) // Branch cycle penalty handled in branch instructions
            }

            AddressingMode::Absolute => {
                let addr = self.fetch16(bus);
                (addr, false)
            }

            AddressingMode::AbsoluteX => {
                let base = self.fetch16(bus);
                let addr = base.wrapping_add(self.x as u16);
                let page_crossed = (base & 0xFF00) != (addr & 0xFF00);
                (addr, page_crossed)
            }

            AddressingMode::AbsoluteY => {
                let base = self.fetch16(bus);
                let addr = base.wrapping_add(self.y as u16);
                let page_crossed = (base & 0xFF00) != (addr & 0xFF00);
                (addr, page_crossed)
            }

            AddressingMode::Indirect => {
                let ptr = self.fetch16(bus);
                let addr = self.read16_wrap(bus, ptr);
                (addr, false)
            }

            AddressingMode::IndirectX => {
                let base = self.fetch(bus);
                let ptr = base.wrapping_add(self.x);
                let lo = bus.read(ptr as u16) as u16;
                let hi = bus.read(ptr.wrapping_add(1) as u16) as u16;
                ((hi << 8) | lo, false)
            }

            AddressingMode::IndirectY => {
                let ptr = self.fetch(bus);
                let lo = bus.read(ptr as u16) as u16;
                let hi = bus.read(ptr.wrapping_add(1) as u16) as u16;
                let base = (hi << 8) | lo;
                let addr = base.wrapping_add(self.y as u16);
                let page_crossed = (base & 0xFF00) != (addr & 0xFF00);
                (addr, page_crossed)
            }
        }
    }
```

- [ ] **Step 2: Write tests for addressing modes**

Add to `mod tests` in `src/cpu.rs`:
```rust
    // Helper: set up CPU at a given PC with bytes in RAM
    fn setup_cpu_with_ram(bytes: &[(u16, u8)]) -> (Cpu, Bus) {
        let mut cpu = Cpu::new();
        let mut bus = Bus::new();
        for &(addr, val) in bytes {
            bus.write(addr, val);
        }
        (cpu, bus)
    }

    #[test]
    fn test_immediate() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[(0x0000, 0x42)]);
        cpu.pc = 0x0000;
        let (addr, _) = cpu.resolve_address(&mut bus, AddressingMode::Immediate);
        assert_eq!(addr, 0x0000);
        assert_eq!(bus.read(addr), 0x42);
        assert_eq!(cpu.pc, 0x0001);
    }

    #[test]
    fn test_zero_page() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[(0x0000, 0x10), (0x0010, 0xAB)]);
        cpu.pc = 0x0000;
        let (addr, _) = cpu.resolve_address(&mut bus, AddressingMode::ZeroPage);
        assert_eq!(addr, 0x0010);
        assert_eq!(bus.read(addr), 0xAB);
    }

    #[test]
    fn test_zero_page_x_wraps() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[(0x0000, 0xFF), (0x0004, 0xBB)]);
        cpu.pc = 0x0000;
        cpu.x = 0x05;
        let (addr, _) = cpu.resolve_address(&mut bus, AddressingMode::ZeroPageX);
        // 0xFF + 0x05 wraps to 0x04 in zero page
        assert_eq!(addr, 0x0004);
    }

    #[test]
    fn test_absolute_x_page_cross() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[(0x0000, 0xFF), (0x0001, 0x00)]);
        cpu.pc = 0x0000;
        cpu.x = 0x01;
        let (addr, page_crossed) = cpu.resolve_address(&mut bus, AddressingMode::AbsoluteX);
        assert_eq!(addr, 0x0100);
        assert!(page_crossed);
    }

    #[test]
    fn test_indirect_y() {
        // Pointer at zero page $10 contains $0300. Y=5. Target = $0305.
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0x10),  // operand: zero page address
            (0x0010, 0x00),  // low byte of pointer
            (0x0011, 0x03),  // high byte of pointer -> $0300
        ]);
        cpu.pc = 0x0000;
        cpu.y = 0x05;
        let (addr, _) = cpu.resolve_address(&mut bus, AddressingMode::IndirectY);
        assert_eq!(addr, 0x0305);
    }

    #[test]
    fn test_indirect_x() {
        // Base=$20, X=4. Pointer at $24 contains $0300.
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0x20),  // operand: zero page base
            (0x0024, 0x00),  // low byte of pointer
            (0x0025, 0x03),  // high byte -> $0300
        ]);
        cpu.pc = 0x0000;
        cpu.x = 0x04;
        let (addr, _) = cpu.resolve_address(&mut bus, AddressingMode::IndirectX);
        assert_eq!(addr, 0x0300);
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml test`
Expected: all tests pass

- [ ] **Step 4: Commit**

```
git add src/cpu.rs
git commit -m "Add 6502 addressing modes with page-crossing detection"
```

### Task 5: CPU — opcode table and step() skeleton

**Files:**
- Modify: `src/cpu.rs`

Build the opcode decode table mapping each opcode to its (instruction, addressing_mode, base_cycles). Implement `step()` that decodes and dispatches. Start with a handful of essential instructions (LDA, STA, JMP, NOP) to validate the framework, then fill in the rest in the next tasks.

- [ ] **Step 1: Add the opcode lookup table and step() method**

Add an `Opcode` struct and lookup function, then implement `step()` with a core set of instructions: LDA (all modes), STA (all modes), LDX, LDY, STX, STY, JMP, JSR, RTS, NOP, SEC, CLC, SEI, CLI, SED, CLD, CLV, TAX, TAY, TXA, TYA, TSX, TXS, PHA, PLA, PHP, PLP, BRK, RTI, INX, INY, DEX, DEY, INC, DEC, AND, ORA, EOR, ADC, SBC, CMP, CPX, CPY, BIT, ASL, LSR, ROL, ROR, BCC, BCS, BEQ, BNE, BMI, BPL, BVC, BVS.

This is the full 6502 official instruction set. The implementation uses a large match on opcode byte. Each arm:
1. Resolves the addressing mode
2. Executes the instruction logic
3. Returns the cycle count

The full code is ~800 lines. Rather than inline it all here, the implementation should follow the NESdev opcode reference at https://www.nesdev.org/obelisk-6502-guide/reference.html.

Key patterns per instruction category:

**Load/Store** (LDA, LDX, LDY, STA, STX, STY):
```rust
// LDA example
let (addr, page_crossed) = self.resolve_address(bus, mode);
self.a = bus.read(addr);
self.update_zero_and_negative(self.a);
// Add 1 cycle if page crossed (for LDA/LDX/LDY only)
```

**Arithmetic** (ADC, SBC):
```rust
// ADC: A + M + C
let (addr, page_crossed) = self.resolve_address(bus, mode);
let operand = bus.read(addr);
let carry = if self.get_flag(CARRY) { 1u16 } else { 0 };
let sum = self.a as u16 + operand as u16 + carry;
self.set_flag(CARRY, sum > 0xFF);
let result = sum as u8;
self.set_flag(OVERFLOW, (self.a ^ result) & (operand ^ result) & 0x80 != 0);
self.a = result;
self.update_zero_and_negative(self.a);
```

**Branches** (BCC, BCS, BEQ, BNE, BMI, BPL, BVC, BVS):
```rust
// Branch template
let (addr, _) = self.resolve_address(bus, AddressingMode::Relative);
if condition {
    let old_pc = self.pc;
    self.pc = addr;
    cycles += 1; // Branch taken
    if (old_pc & 0xFF00) != (addr & 0xFF00) {
        cycles += 1; // Page crossed
    }
}
```

**Shifts** (ASL, LSR, ROL, ROR):
```rust
// ASL accumulator vs memory
if mode == AddressingMode::Accumulator {
    self.set_flag(CARRY, self.a & 0x80 != 0);
    self.a <<= 1;
    self.update_zero_and_negative(self.a);
} else {
    let (addr, _) = self.resolve_address(bus, mode);
    let mut val = bus.read(addr);
    self.set_flag(CARRY, val & 0x80 != 0);
    val <<= 1;
    bus.write(addr, val);
    self.update_zero_and_negative(val);
}
```

The full `step()` method signature:
```rust
    /// Execute one instruction. Returns the number of CPU cycles consumed.
    pub fn step(&mut self, bus: &mut Bus) -> u8 {
        let opcode = self.fetch(bus);
        match opcode {
            // 0x00 BRK
            // 0x01 ORA (Indirect,X)
            // ... all 151 valid opcodes ...
            // 0xEA NOP
            _ => panic!("Unimplemented opcode: 0x{:02X} at PC=0x{:04X}", opcode, self.pc - 1),
        }
    }
```

Also add NMI and IRQ handlers:
```rust
    /// Non-maskable interrupt (triggered by PPU VBlank)
    pub fn nmi(&mut self, bus: &mut Bus) {
        self.push16(bus, self.pc);
        self.push(bus, (self.status | UNUSED) & !BREAK);
        self.set_flag(INTERRUPT_DISABLE, true);
        let lo = bus.read(0xFFFA) as u16;
        let hi = bus.read(0xFFFB) as u16;
        self.pc = (hi << 8) | lo;
        self.cycles += 7;
    }

    /// Maskable interrupt
    pub fn irq(&mut self, bus: &mut Bus) {
        if !self.get_flag(INTERRUPT_DISABLE) {
            self.push16(bus, self.pc);
            self.push(bus, (self.status | UNUSED) & !BREAK);
            self.set_flag(INTERRUPT_DISABLE, true);
            let lo = bus.read(0xFFFE) as u16;
            let hi = bus.read(0xFFFF) as u16;
            self.pc = (hi << 8) | lo;
            self.cycles += 7;
        }
    }
```

- [ ] **Step 2: Write tests for basic instruction execution**

```rust
    #[test]
    fn test_lda_immediate() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0xA9), // LDA #imm
            (0x0001, 0x42), // operand
        ]);
        cpu.pc = 0x0000;
        let cycles = cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x42);
        assert_eq!(cycles, 2);
        assert!(!cpu.get_flag(ZERO));
        assert!(!cpu.get_flag(NEGATIVE));
    }

    #[test]
    fn test_lda_zero_flag() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0xA9),
            (0x0001, 0x00),
        ]);
        cpu.pc = 0x0000;
        cpu.step(&mut bus);
        assert!(cpu.get_flag(ZERO));
    }

    #[test]
    fn test_sta_zero_page() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0x85), // STA zp
            (0x0001, 0x10), // address $10
        ]);
        cpu.pc = 0x0000;
        cpu.a = 0x42;
        let cycles = cpu.step(&mut bus);
        assert_eq!(bus.read(0x0010), 0x42);
        assert_eq!(cycles, 3);
    }

    #[test]
    fn test_jmp_absolute() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0x4C), // JMP abs
            (0x0001, 0x00), // low byte
            (0x0002, 0x06), // high byte -> $0600
        ]);
        cpu.pc = 0x0000;
        cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x0600);
    }

    #[test]
    fn test_adc_with_carry() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0x69), // ADC #imm
            (0x0001, 0x01),
        ]);
        cpu.pc = 0x0000;
        cpu.a = 0xFF;
        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.get_flag(CARRY));
        assert!(cpu.get_flag(ZERO));
    }

    #[test]
    fn test_adc_overflow() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0x69), // ADC #imm
            (0x0001, 0x50), // 80 + 80 = 160, signed overflow
        ]);
        cpu.pc = 0x0000;
        cpu.a = 0x50;
        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0xA0);
        assert!(cpu.get_flag(OVERFLOW));
        assert!(cpu.get_flag(NEGATIVE));
    }

    #[test]
    fn test_inx_wraps() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[(0x0000, 0xE8)]); // INX
        cpu.pc = 0x0000;
        cpu.x = 0xFF;
        cpu.step(&mut bus);
        assert_eq!(cpu.x, 0x00);
        assert!(cpu.get_flag(ZERO));
    }

    #[test]
    fn test_bne_taken() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0xD0), // BNE
            (0x0001, 0x02), // offset +2
        ]);
        cpu.pc = 0x0000;
        cpu.set_flag(ZERO, false);
        let cycles = cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x0004); // $0002 + 2
        assert_eq!(cycles, 3); // 2 base + 1 branch taken
    }

    #[test]
    fn test_bne_not_taken() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0xD0), // BNE
            (0x0001, 0x02),
        ]);
        cpu.pc = 0x0000;
        cpu.set_flag(ZERO, true);
        let cycles = cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x0002);
        assert_eq!(cycles, 2);
    }

    #[test]
    fn test_jsr_rts() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0x20), // JSR $0600
            (0x0001, 0x00),
            (0x0002, 0x06),
            (0x0600, 0x60), // RTS
        ]);
        cpu.pc = 0x0000;
        cpu.sp = 0xFF;
        cpu.step(&mut bus);       // JSR
        assert_eq!(cpu.pc, 0x0600);
        cpu.step(&mut bus);       // RTS
        assert_eq!(cpu.pc, 0x0003);
    }

    #[test]
    fn test_pha_pla() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0x48), // PHA
            (0x0001, 0xA9), // LDA #0
            (0x0002, 0x00),
            (0x0003, 0x68), // PLA
        ]);
        cpu.pc = 0x0000;
        cpu.sp = 0xFF;
        cpu.a = 0x42;
        cpu.step(&mut bus); // PHA
        cpu.step(&mut bus); // LDA #0
        assert_eq!(cpu.a, 0x00);
        cpu.step(&mut bus); // PLA
        assert_eq!(cpu.a, 0x42);
    }

    #[test]
    fn test_nmi() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x00FA, 0x00), // NMI vector low (in RAM, mirrored — for test only)
            (0x00FB, 0x06), // NMI vector high -> $0600
        ]);
        cpu.pc = 0x0200;
        cpu.sp = 0xFF;
        // NMI vector is at $FFFA-$FFFB which is cartridge space.
        // For this unit test, we rely on Bus returning 0 for unmapped reads.
        // Full NMI test will work after cartridge is wired up.
        cpu.nmi(&mut bus);
        assert_eq!(cpu.sp, 0xFC); // pushed PC (2 bytes) + status (1 byte)
        assert!(cpu.get_flag(INTERRUPT_DISABLE));
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml test`
Expected: all tests pass

- [ ] **Step 4: Commit**

```
git add src/cpu.rs
git commit -m "Implement full 6502 instruction set with step() dispatch"
```

---

## Milestone 2: iNES Parser + Mapper 0

### Task 6: Mapper trait and Mapper 0

**Files:**
- Create: `src/mapper/mod.rs`
- Create: `src/mapper/mapper0.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create Mapper trait and Mapper 0 implementation**

`src/mapper/mod.rs`:
```rust
pub mod mapper0;

pub trait Mapper {
    fn cpu_read(&self, addr: u16) -> u8;
    fn cpu_write(&mut self, addr: u16, data: u8);
    fn ppu_read(&self, addr: u16) -> u8;
    fn ppu_write(&mut self, addr: u16, data: u8);
}
```

`src/mapper/mapper0.rs`:
```rust
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
                    index %= 16384; // Mirror 16KB
                }
                self.prg_rom[index]
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, _addr: u16, _data: u8) {
        // NROM has no writable registers
    }

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

    fn ppu_write(&mut self, _addr: u16, _data: u8) {
        // CHR ROM is read-only for Mapper 0
    }
}
```

- [ ] **Step 2: Add mapper module to main.rs**

```rust
mod bus;
mod cpu;
mod mapper;

fn main() {
    println!("Hello, world!");
}
```

- [ ] **Step 3: Write tests for Mapper 0**

Add to `src/mapper/mapper0.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mapper0_32kb_prg() {
        let prg = vec![0u8; 32768];
        let chr = vec![0u8; 8192];
        let mut mapper = Mapper0::new(prg, chr);
        // First byte
        assert_eq!(mapper.cpu_read(0x8000), 0);
        // Write has no effect
        mapper.cpu_write(0x8000, 0xFF);
        assert_eq!(mapper.cpu_read(0x8000), 0);
    }

    #[test]
    fn test_mapper0_16kb_mirror() {
        let mut prg = vec![0u8; 16384];
        prg[0] = 0xAA;
        let chr = vec![0u8; 8192];
        let mapper = Mapper0::new(prg, chr);
        // $8000 and $C000 should both read 0xAA
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
```

- [ ] **Step 4: Run tests**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml test`
Expected: all tests pass

- [ ] **Step 5: Commit**

```
git add src/mapper/
git commit -m "Add Mapper trait and Mapper 0 (NROM) implementation"
```

### Task 7: Cartridge — iNES file parser

**Files:**
- Create: `src/cartridge.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Implement iNES parser and Cartridge struct**

`src/cartridge.rs`:
```rust
use crate::mapper::mapper0::Mapper0;
use crate::mapper::Mapper;

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
    /// Load from iNES format (.nes file)
    pub fn from_ines(data: &[u8]) -> Result<Self, String> {
        // Verify header magic: "NES\x1A"
        if data.len() < 16 || &data[0..4] != b"NES\x1a" {
            return Err("Invalid iNES header".into());
        }

        let prg_rom_size = data[4] as usize * 16384; // 16KB units
        let chr_rom_size = data[5] as usize * 8192;   // 8KB units
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
            _ => return Err(format!("Unsupported mapper: {}", mapper_number)),
        };

        Ok(Cartridge { mapper, mirroring })
    }
}
```

- [ ] **Step 2: Add cartridge module to main.rs**

```rust
mod bus;
mod cartridge;
mod cpu;
mod mapper;

fn main() {
    println!("Hello, world!");
}
```

- [ ] **Step 3: Write tests for iNES parser**

Add to `src/cartridge.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_ines_header(prg_banks: u8, chr_banks: u8, flags6: u8, flags7: u8) -> Vec<u8> {
        let mut header = vec![0x4E, 0x45, 0x53, 0x1A]; // "NES\x1A"
        header.push(prg_banks);
        header.push(chr_banks);
        header.push(flags6);
        header.push(flags7);
        header.extend_from_slice(&[0u8; 8]); // Rest of header
        // PRG ROM
        header.extend(vec![0u8; prg_banks as usize * 16384]);
        // CHR ROM
        header.extend(vec![0u8; chr_banks as usize * 8192]);
        header
    }

    #[test]
    fn test_parse_valid_ines() {
        let data = make_ines_header(2, 1, 0x01, 0x00); // Mapper 0, vertical mirroring
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
        let data = make_ines_header(1, 1, 0x10, 0x00); // Mapper 1
        assert!(Cartridge::from_ines(&data).is_err());
    }

    #[test]
    fn test_chr_ram_when_no_chr_rom() {
        let data = make_ines_header(1, 0, 0x00, 0x00); // 0 CHR banks
        let cart = Cartridge::from_ines(&data).unwrap();
        // Should still be able to read CHR (RAM, initialized to 0)
        assert_eq!(cart.mapper.ppu_read(0x0000), 0);
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml test`
Expected: all tests pass

- [ ] **Step 5: Commit**

```
git add src/cartridge.rs src/main.rs
git commit -m "Add iNES file parser and Cartridge struct"
```

### Task 8: Wire Cartridge into Bus

**Files:**
- Modify: `src/bus.rs`

- [ ] **Step 1: Add Cartridge to Bus and route cartridge address space**

Update `Bus` to hold an `Option<Cartridge>` (so Bus can exist without a loaded ROM for testing):

```rust
use crate::cartridge::Cartridge;

pub struct Bus {
    ram: [u8; 2048],
    pub cartridge: Option<Cartridge>,
}

impl Bus {
    pub fn new() -> Self {
        Self {
            ram: [0; 2048],
            cartridge: None,
        }
    }

    pub fn load_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            0x2000..=0x3FFF => 0, // PPU stub
            0x4000..=0x4017 => 0, // APU/IO stub
            0x4020..=0xFFFF => {
                if let Some(ref cart) = self.cartridge {
                    cart.mapper.cpu_read(addr)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize] = data,
            0x2000..=0x3FFF => {} // PPU stub
            0x4000..=0x4017 => {} // APU/IO stub
            0x4020..=0xFFFF => {
                if let Some(ref mut cart) = self.cartridge {
                    cart.mapper.cpu_write(addr, data);
                }
            }
            _ => {}
        }
    }
}
```

- [ ] **Step 2: Write test loading a cartridge and reading from it**

Add test to `src/bus.rs`:
```rust
    #[test]
    fn test_cartridge_read() {
        let mut bus = Bus::new();
        // Create a minimal cartridge with known data
        let mut prg = vec![0u8; 32768];
        prg[0] = 0xEA; // NOP at $8000
        prg[0x7FFC] = 0x00; // Reset vector low ($FFFC)
        prg[0x7FFD] = 0x80; // Reset vector high -> $8000
        let chr = vec![0u8; 8192];
        let cart = crate::cartridge::Cartridge {
            mapper: Box::new(crate::mapper::mapper0::Mapper0::new(prg, chr)),
            mirroring: crate::cartridge::Mirroring::Horizontal,
        };
        bus.load_cartridge(cart);

        assert_eq!(bus.read(0x8000), 0xEA);
        assert_eq!(bus.read(0xFFFC), 0x00);
        assert_eq!(bus.read(0xFFFD), 0x80);
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml test`
Expected: all tests pass

- [ ] **Step 4: Commit**

```
git add src/bus.rs
git commit -m "Wire Cartridge into Bus for cartridge address space routing"
```

### Task 9: Download nestest and verify loading

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Download nestest.nes and nestest.log**

```bash
curl -L -o roms/nestest.nes "https://nickmass.com/images/nestest.nes"
curl -L -o roms/nestest.log "https://www.qmtpro.com/~nes/misc/nestest.log"
```

- [ ] **Step 2: Write a quick integration test that loads nestest**

Create `tests/nestest.rs`:
```rust
use std::fs;

#[test]
fn test_load_nestest() {
    let data = fs::read("roms/nestest.nes").expect("nestest.nes not found in roms/");
    let cart = rfc::cartridge::Cartridge::from_ines(&data).unwrap();
    assert_eq!(cart.mirroring, rfc::cartridge::Mirroring::Horizontal);
}
```

This requires making modules public. Update `src/main.rs` to use `pub mod`:
```rust
pub mod bus;
pub mod cartridge;
pub mod cpu;
pub mod mapper;

fn main() {
    println!("Hello, world!");
}
```

Also add `lib.rs` to re-export for integration tests:

Create `src/lib.rs`:
```rust
pub mod bus;
pub mod cartridge;
pub mod cpu;
pub mod mapper;
```

And remove the `mod` declarations from `main.rs`, using `use` instead:
```rust
use rfc::bus::Bus;
use rfc::cartridge::Cartridge;
use rfc::cpu::Cpu;

fn main() {
    println!("Hello, world!");
}
```

- [ ] **Step 3: Run tests**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml test`
Expected: all tests pass including the integration test

- [ ] **Step 4: Commit**

```
git add src/lib.rs src/main.rs tests/nestest.rs
git commit -m "Add lib.rs, verify nestest.nes loads correctly"
```

---

## Milestone 3: Complete CPU — Pass nestest

### Task 10: nestest log comparison harness

**Files:**
- Create: `tests/nestest_cpu.rs`

The nestest.log file contains expected CPU state after each instruction. Format per line:
```
C000  4C F5 C5  JMP $C5F5                       A:00 X:00 Y:00 P:24 SP:FD PPU:  0, 21 CYC:7
```

We build a test that runs the CPU and compares state line-by-line.

- [ ] **Step 1: Write the nestest comparison test**

`tests/nestest_cpu.rs`:
```rust
use std::fs;

/// Parse one line of nestest.log to extract PC, A, X, Y, P, SP
fn parse_log_line(line: &str) -> (u16, u8, u8, u8, u8, u8) {
    let pc = u16::from_str_radix(&line[0..4], 16).unwrap();
    // Find register values after the instruction disassembly
    let a_pos = line.find("A:").unwrap();
    let a = u8::from_str_radix(&line[a_pos+2..a_pos+4], 16).unwrap();
    let x_pos = line.find("X:").unwrap();
    let x = u8::from_str_radix(&line[x_pos+2..x_pos+4], 16).unwrap();
    let y_pos = line.find("Y:").unwrap();
    let y = u8::from_str_radix(&line[y_pos+2..y_pos+4], 16).unwrap();
    let p_pos = line.find("P:").unwrap();
    let p = u8::from_str_radix(&line[p_pos+2..p_pos+4], 16).unwrap();
    let sp_pos = line.find("SP:").unwrap();
    let sp = u8::from_str_radix(&line[sp_pos+3..sp_pos+5], 16).unwrap();
    (pc, a, x, y, p, sp)
}

#[test]
fn test_nestest_cpu() {
    let rom_data = fs::read("roms/nestest.nes").expect("nestest.nes not found");
    let log_data = fs::read_to_string("roms/nestest.log").expect("nestest.log not found");

    let cart = rfc::cartridge::Cartridge::from_ines(&rom_data).unwrap();
    let mut bus = rfc::bus::Bus::new();
    bus.load_cartridge(cart);

    let mut cpu = rfc::cpu::Cpu::new();
    // nestest starts at $C000 in automation mode
    cpu.pc = 0xC000;
    cpu.status = 0x24; // nestest expects this initial status
    cpu.cycles = 7;

    let log_lines: Vec<&str> = log_data.lines().collect();

    for (i, expected_line) in log_lines.iter().enumerate() {
        let (exp_pc, exp_a, exp_x, exp_y, exp_p, exp_sp) = parse_log_line(expected_line);

        // Compare state BEFORE executing the instruction
        assert_eq!(
            cpu.pc, exp_pc,
            "Line {}: PC mismatch: got 0x{:04X}, expected 0x{:04X}",
            i + 1, cpu.pc, exp_pc
        );
        assert_eq!(
            cpu.a, exp_a,
            "Line {}: A mismatch: got 0x{:02X}, expected 0x{:02X}",
            i + 1, cpu.a, exp_a
        );
        assert_eq!(
            cpu.x, exp_x,
            "Line {}: X mismatch: got 0x{:02X}, expected 0x{:02X}",
            i + 1, cpu.x, exp_x
        );
        assert_eq!(
            cpu.y, exp_y,
            "Line {}: Y mismatch: got 0x{:02X}, expected 0x{:02X}",
            i + 1, cpu.y, exp_y
        );
        assert_eq!(
            cpu.status, exp_p,
            "Line {}: P mismatch: got 0x{:02X}, expected 0x{:02X}",
            i + 1, cpu.status, exp_p
        );
        assert_eq!(
            cpu.sp, exp_sp,
            "Line {}: SP mismatch: got 0x{:02X}, expected 0x{:02X}",
            i + 1, cpu.sp, exp_sp
        );

        cpu.step(&mut bus);
    }

    println!("nestest passed all {} lines!", log_lines.len());
}
```

- [ ] **Step 2: Run the test — expect it to fail initially, then iterate on CPU bugs**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml test nestest_cpu -- --nocapture`

The test will likely fail on specific instructions. Use the line number and opcode to identify which instruction implementation has bugs. Fix them iteratively until the full log passes.

Common issues to watch for:
- BRK pushes PC+2 (not PC+1)
- PHP pushes status with BREAK and UNUSED bits set
- PLP ignores BREAK bit, always sets UNUSED
- ROR/ROL carry handling
- SBC is ADC with complement (~operand)
- Indirect JMP page-boundary bug ($xxFF wraps within page)

- [ ] **Step 3: Keep fixing until nestest passes completely**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml test nestest_cpu -- --nocapture`
Expected: all lines pass, test prints "nestest passed all N lines!"

- [ ] **Step 4: Commit**

```
git add tests/nestest_cpu.rs src/cpu.rs
git commit -m "Pass nestest: complete CPU instruction set verified"
```

---

## Milestone 4: PPU Basic Rendering

### Task 11: PPU struct and register I/O

**Files:**
- Create: `src/ppu.rs`
- Modify: `src/bus.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create PPU struct with all registers**

`src/ppu.rs`:
```rust
use crate::cartridge::Mirroring;
use crate::mapper::Mapper;

pub const SCREEN_WIDTH: usize = 256;
pub const SCREEN_HEIGHT: usize = 240;

pub struct Ppu {
    // VRAM (2KB nametable memory)
    vram: [u8; 2048],
    // OAM (sprite attribute memory)
    pub oam: [u8; 256],
    // Palette RAM
    palette: [u8; 32],

    // Registers
    pub ctrl: u8,       // $2000 PPUCTRL
    pub mask: u8,       // $2001 PPUMASK
    pub status: u8,     // $2002 PPUSTATUS
    pub oam_addr: u8,   // $2003 OAMADDR

    // Internal registers for scrolling (loopy registers)
    v: u16,             // Current VRAM address (15 bits)
    t: u16,             // Temporary VRAM address (15 bits)
    x: u8,              // Fine X scroll (3 bits)
    w: bool,            // Write toggle (first/second write)

    // Data read buffer ($2007)
    data_buffer: u8,

    // Rendering state
    pub scanline: u16,
    pub cycle: u16,
    pub frame_buffer: [u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4], // RGBA

    // Signals
    pub nmi_pending: bool,
    pub frame_complete: bool,

    // Mirroring mode (from cartridge)
    mirroring: Mirroring,
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
            frame_buffer: [0; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
            nmi_pending: false,
            frame_complete: false,
            mirroring,
        }
    }

    /// CPU writes to PPU register ($2000-$2007)
    pub fn write_register(&mut self, addr: u16, data: u8, mapper: &mut dyn Mapper) {
        match addr {
            0x2000 => { // PPUCTRL
                let before_nmi = self.ctrl & 0x80 != 0;
                self.ctrl = data;
                // t: ...GH.. ........ <- d: ......GH (nametable select)
                self.t = (self.t & 0xF3FF) | ((data as u16 & 0x03) << 10);
                // If NMI enabled and VBlank is set, trigger NMI
                let after_nmi = data & 0x80 != 0;
                if !before_nmi && after_nmi && self.status & 0x80 != 0 {
                    self.nmi_pending = true;
                }
            }
            0x2001 => self.mask = data, // PPUMASK
            0x2003 => self.oam_addr = data, // OAMADDR
            0x2004 => { // OAMDATA write
                self.oam[self.oam_addr as usize] = data;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            0x2005 => { // PPUSCROLL
                if !self.w {
                    // First write: X scroll
                    self.t = (self.t & 0xFFE0) | (data as u16 >> 3);
                    self.x = data & 0x07;
                    self.w = true;
                } else {
                    // Second write: Y scroll
                    self.t = (self.t & 0x8C1F)
                        | ((data as u16 & 0x07) << 12)
                        | ((data as u16 & 0xF8) << 2);
                    self.w = false;
                }
            }
            0x2006 => { // PPUADDR
                if !self.w {
                    // First write: high byte
                    self.t = (self.t & 0x00FF) | ((data as u16 & 0x3F) << 8);
                    self.w = true;
                } else {
                    // Second write: low byte
                    self.t = (self.t & 0xFF00) | data as u16;
                    self.v = self.t;
                    self.w = false;
                }
            }
            0x2007 => { // PPUDATA write
                self.ppu_write(self.v, data, mapper);
                self.v = self.v.wrapping_add(if self.ctrl & 0x04 != 0 { 32 } else { 1 });
            }
            _ => {}
        }
    }

    /// CPU reads from PPU register ($2000-$2007)
    pub fn read_register(&mut self, addr: u16, mapper: &dyn Mapper) -> u8 {
        match addr {
            0x2002 => { // PPUSTATUS
                let data = (self.status & 0xE0) | (self.data_buffer & 0x1F);
                self.status &= !0x80; // Clear VBlank flag
                self.w = false; // Reset write latch
                data
            }
            0x2004 => self.oam[self.oam_addr as usize], // OAMDATA read
            0x2007 => { // PPUDATA read
                let addr = self.v;
                self.v = self.v.wrapping_add(if self.ctrl & 0x04 != 0 { 32 } else { 1 });
                if addr < 0x3F00 {
                    // Buffered read
                    let data = self.data_buffer;
                    self.data_buffer = self.ppu_read(addr, mapper);
                    data
                } else {
                    // Palette reads are not buffered
                    self.data_buffer = self.ppu_read(addr - 0x1000, mapper);
                    self.ppu_read(addr, mapper)
                }
            }
            _ => 0,
        }
    }

    /// Internal PPU read (VRAM, palette, or CHR via mapper)
    fn ppu_read(&self, addr: u16, mapper: &dyn Mapper) -> u8 {
        let addr = addr & 0x3FFF; // Mirror above $3FFF
        match addr {
            0x0000..=0x1FFF => mapper.ppu_read(addr),
            0x2000..=0x3EFF => {
                let mirrored = self.mirror_nametable(addr);
                self.vram[mirrored]
            }
            0x3F00..=0x3FFF => {
                let mut index = (addr & 0x1F) as usize;
                // Mirrors of background color
                if index == 0x10 || index == 0x14 || index == 0x18 || index == 0x1C {
                    index -= 0x10;
                }
                self.palette[index]
            }
            _ => 0,
        }
    }

    /// Internal PPU write
    fn ppu_write(&mut self, addr: u16, data: u8, mapper: &mut dyn Mapper) {
        let addr = addr & 0x3FFF;
        match addr {
            0x0000..=0x1FFF => mapper.ppu_write(addr, data),
            0x2000..=0x3EFF => {
                let mirrored = self.mirror_nametable(addr);
                self.vram[mirrored] = data;
            }
            0x3F00..=0x3FFF => {
                let mut index = (addr & 0x1F) as usize;
                if index == 0x10 || index == 0x14 || index == 0x18 || index == 0x1C {
                    index -= 0x10;
                }
                self.palette[index] = data;
            }
            _ => {}
        }
    }

    /// Map nametable address to VRAM index based on mirroring mode
    fn mirror_nametable(&self, addr: u16) -> usize {
        let addr = (addr - 0x2000) & 0x0FFF;
        let table = addr / 0x0400;
        let offset = addr % 0x0400;
        let mapped_table = match self.mirroring {
            Mirroring::Horizontal => match table {
                0 | 1 => 0,
                2 | 3 => 1,
                _ => unreachable!(),
            },
            Mirroring::Vertical => match table {
                0 | 2 => 0,
                1 | 3 => 1,
                _ => unreachable!(),
            },
            Mirroring::FourScreen => table as usize,
        };
        (mapped_table * 0x0400 + offset as usize) as usize
    }
}
```

- [ ] **Step 2: Wire PPU into Bus**

Update `src/bus.rs` to hold PPU and route $2000-$3FFF:

Add PPU to Bus struct, route read/write for PPU registers. PPU register access needs the mapper for $2007 reads/writes, so Bus passes mapper through.

- [ ] **Step 3: Write tests for PPU register I/O**

Test PPUCTRL write, PPUSTATUS read (VBlank clear + latch reset), PPUADDR double write, PPUDATA buffered read, nametable mirroring.

- [ ] **Step 4: Run tests**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml test`
Expected: all tests pass

- [ ] **Step 5: Commit**

```
git add src/ppu.rs src/bus.rs src/lib.rs
git commit -m "Add PPU struct with register I/O and nametable mirroring"
```

### Task 12: PPU scanline rendering engine

**Files:**
- Modify: `src/ppu.rs`

- [ ] **Step 1: Implement the scanline-based rendering loop**

Add to `impl Ppu`:

```rust
    /// NES master palette — 64 colors as RGBA
    const PALETTE_COLORS: [(u8, u8, u8); 64] = [
        (84,84,84),    (0,30,116),    (8,16,144),    (48,0,136),
        (68,0,100),    (92,0,48),     (84,4,0),      (60,24,0),
        (32,42,0),     (8,58,0),      (0,64,0),      (0,60,0),
        (0,50,60),     (0,0,0),       (0,0,0),       (0,0,0),
        (152,150,152), (8,76,196),    (48,50,236),   (92,30,228),
        (136,20,176),  (160,20,100),  (152,34,32),   (120,60,0),
        (84,90,0),     (40,114,0),    (8,124,0),     (0,118,40),
        (0,102,120),   (0,0,0),       (0,0,0),       (0,0,0),
        (236,238,236), (76,154,236),  (120,124,236), (176,98,236),
        (228,84,236),  (236,88,180),  (236,106,100), (212,136,32),
        (160,170,0),   (116,196,0),   (76,208,32),   (56,204,108),
        (56,180,204),  (60,60,60),    (0,0,0),       (0,0,0),
        (236,238,236), (168,204,236), (188,188,236), (212,178,236),
        (236,174,236), (236,174,212), (236,180,176), (228,196,144),
        (204,210,120), (180,222,120), (168,226,144), (152,226,180),
        (160,214,228), (160,162,160), (0,0,0),       (0,0,0),
    ];

    /// Advance PPU by one cycle. Call this N*3 times per CPU cycle.
    pub fn step(&mut self, mapper: &dyn Mapper) {
        // Visible scanlines (0-239)
        if self.scanline < 240 && self.cycle >= 1 && self.cycle <= 256 {
            self.render_pixel(mapper);
        }

        // End of visible scanline — evaluate sprites for next line
        if self.scanline < 240 && self.cycle == 257 {
            // Sprite evaluation happens here (simplified)
        }

        // VBlank start (scanline 241, cycle 1)
        if self.scanline == 241 && self.cycle == 1 {
            self.status |= 0x80; // Set VBlank flag
            if self.ctrl & 0x80 != 0 {
                self.nmi_pending = true;
            }
            self.frame_complete = true;
        }

        // Pre-render scanline (261)
        if self.scanline == 261 && self.cycle == 1 {
            self.status &= !0x80; // Clear VBlank
            self.status &= !0x40; // Clear sprite 0 hit
            self.status &= !0x20; // Clear sprite overflow
            self.nmi_pending = false;
        }

        // Advance cycle/scanline counters
        self.cycle += 1;
        if self.cycle > 340 {
            self.cycle = 0;
            self.scanline += 1;
            if self.scanline > 261 {
                self.scanline = 0;
                self.frame_complete = false;
            }
        }
    }

    /// Render one pixel at the current scanline/cycle position
    fn render_pixel(&self, mapper: &dyn Mapper) {
        let x = (self.cycle - 1) as usize;
        let y = self.scanline as usize;

        if !self.rendering_enabled() {
            self.set_pixel(x, y, 0); // Background color
            return;
        }

        // Background pixel
        let bg_color = if self.mask & 0x08 != 0 {
            self.get_background_pixel(x, mapper)
        } else {
            0
        };

        // Sprite pixel (will be implemented in Task 14)
        // For now, just use background
        let color_index = if bg_color % 4 == 0 { 0 } else { bg_color };
        let palette_index = self.ppu_read_palette(color_index as u16);
        self.set_pixel(x, y, palette_index);
    }

    fn rendering_enabled(&self) -> bool {
        self.mask & 0x18 != 0
    }

    fn get_background_pixel(&self, screen_x: usize, mapper: &dyn Mapper) -> u8 {
        let scroll_x = screen_x as u16 + (self.x as u16);
        // Use v register for current scroll position
        let v = self.v;
        let fine_y = (v >> 12) & 0x07;
        let nametable = 0x2000 | (v & 0x0FFF);
        let coarse_x = v & 0x001F;
        let coarse_y = (v >> 5) & 0x001F;

        // Nametable byte
        let tile_addr = 0x2000 | (v & 0x0FFF);
        let tile_index = self.ppu_read(tile_addr & 0x3FFF, mapper);

        // Pattern table address
        let pattern_table = if self.ctrl & 0x10 != 0 { 0x1000 } else { 0x0000 };
        let pattern_addr = pattern_table + tile_index as u16 * 16 + fine_y;
        let pattern_lo = mapper.ppu_read(pattern_addr);
        let pattern_hi = mapper.ppu_read(pattern_addr + 8);

        // Pixel bit (bit 7 = leftmost pixel)
        let bit = 7 - self.x;
        let pixel_lo = (pattern_lo >> bit) & 1;
        let pixel_hi = (pattern_hi >> bit) & 1;
        let pixel = pixel_lo | (pixel_hi << 1);

        // Attribute byte
        let attr_addr = 0x23C0 | (v & 0x0C00) | ((coarse_y >> 2) << 3) | (coarse_x >> 2);
        let attr_byte = self.ppu_read(attr_addr, mapper);
        let shift = ((coarse_y & 0x02) | ((coarse_x & 0x02) >> 1)) * 2;
        let palette_num = (attr_byte >> shift) & 0x03;

        if pixel == 0 {
            0 // Transparent (background color)
        } else {
            (palette_num << 2) | pixel
        }
    }

    fn ppu_read_palette(&self, addr: u16) -> u8 {
        let mut index = (addr & 0x1F) as usize;
        if index == 0x10 || index == 0x14 || index == 0x18 || index == 0x1C {
            index -= 0x10;
        }
        self.palette[index]
    }

    fn set_pixel(&self, x: usize, y: usize, palette_index: u8) {
        if x >= SCREEN_WIDTH || y >= SCREEN_HEIGHT {
            return;
        }
        let (r, g, b) = Self::PALETTE_COLORS[(palette_index & 0x3F) as usize];
        let offset = (y * SCREEN_WIDTH + x) * 4;
        // Safety: frame_buffer is written from PPU step, read from renderer
        // In single-threaded execution this is fine.
        let fb = unsafe {
            &mut *(&self.frame_buffer as *const _ as *mut [u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4])
        };
        fb[offset] = r;
        fb[offset + 1] = g;
        fb[offset + 2] = b;
        fb[offset + 3] = 255;
    }
```

Note: The background rendering above is simplified. The proper implementation needs to handle the loopy register scroll mechanics (increment X/Y at correct cycle boundaries). This will be refined when testing against actual ROMs. The key point is the structure is in place.

- [ ] **Step 2: Run tests (compile check)**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml check`
Expected: compiles successfully

- [ ] **Step 3: Commit**

```
git add src/ppu.rs
git commit -m "Add PPU scanline rendering engine with background tile rendering"
```

### Task 13: Console and main loop — render a frame

**Files:**
- Create: `src/console.rs`
- Create: `src/renderer.rs`
- Modify: `src/main.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create Console that drives CPU+PPU synchronization**

`src/console.rs`:
```rust
use crate::bus::Bus;
use crate::cpu::Cpu;

pub struct Console {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl Console {
    pub fn new(bus: Bus) -> Self {
        let cpu = Cpu::new();
        Self { cpu, bus }
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
    }

    /// Run until one frame is complete
    pub fn step_frame(&mut self) {
        loop {
            let cpu_cycles = self.cpu.step(&mut self.bus) as u16;

            // PPU runs 3× faster than CPU
            let ppu_cycles = cpu_cycles * 3;
            for _ in 0..ppu_cycles {
                self.bus.step_ppu();
            }

            // Check for NMI from PPU
            if self.bus.poll_nmi() {
                self.cpu.nmi(&mut self.bus);
            }

            if self.bus.frame_complete() {
                break;
            }
        }
    }

    pub fn frame_buffer(&self) -> &[u8] {
        self.bus.frame_buffer()
    }
}
```

This requires adding `step_ppu()`, `poll_nmi()`, `frame_complete()`, and `frame_buffer()` methods to Bus, which delegate to PPU.

- [ ] **Step 2: Create renderer with winit + wgpu**

`src/renderer.rs` — creates a window, sets up a wgpu pipeline that renders a texture to a full-screen quad. Each frame, uploads `frame_buffer` as the texture source.

This is ~200 lines of wgpu boilerplate. Key structure:
- `Renderer::new(window)` — init wgpu device, create texture, create render pipeline
- `Renderer::render(frame_buffer)` — upload pixels to texture, draw quad
- Vertex shader: pass-through quad
- Fragment shader: sample from texture

- [ ] **Step 3: Wire up main.rs with event loop**

`src/main.rs`:
```rust
use rfc::bus::Bus;
use rfc::cartridge::Cartridge;
use rfc::console::Console;
use rfc::renderer::Renderer;
use std::fs;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;

fn main() {
    env_logger::init();

    // Load ROM (hardcoded path for now, config comes later)
    let rom_path = std::env::args().nth(1).unwrap_or_else(|| "roms/nestest.nes".into());
    let rom_data = fs::read(&rom_path).expect("Failed to read ROM file");
    let cartridge = Cartridge::from_ines(&rom_data).expect("Failed to parse ROM");

    let mut bus = Bus::new();
    bus.load_cartridge(cartridge);
    let mut console = Console::new(bus);
    console.reset();

    // Create window and run event loop
    let event_loop = EventLoop::new().unwrap();
    // ... winit + wgpu setup, call console.step_frame() per frame,
    // upload frame_buffer to renderer
}
```

- [ ] **Step 4: Test by running with nestest.nes — should show some pixels on screen**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml run -- roms/nestest.nes`
Expected: a window opens showing some rendering (likely garbled at this stage, but pixels appear)

- [ ] **Step 5: Commit**

```
git add src/console.rs src/renderer.rs src/main.rs src/lib.rs
git commit -m "Add Console, wgpu renderer, and main event loop — first pixels on screen"
```

### Task 14: PPU sprite rendering and sprite-0 hit

**Files:**
- Modify: `src/ppu.rs`

- [ ] **Step 1: Implement sprite evaluation and rendering**

Add sprite rendering to the PPU. For each visible scanline:
1. Evaluate which sprites (up to 8) are on this scanline
2. For each sprite, fetch pattern data
3. In `render_pixel()`, compare sprite vs background priority, apply sprite-0 hit detection

- [ ] **Step 2: Test with SMB ROM**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml run -- roms/<smb.nes>`
Expected: SMB title screen renders with Mario logo and menu text visible

- [ ] **Step 3: Commit**

```
git add src/ppu.rs
git commit -m "Add PPU sprite rendering and sprite-0 hit detection"
```

---

## Milestone 5: PPU Scrolling

### Task 15: PPU scroll register mechanics (loopy)

**Files:**
- Modify: `src/ppu.rs`

- [ ] **Step 1: Implement proper loopy register scroll mechanics**

The NES PPU uses internal registers (v, t, x, w) for scroll position. During rendering:
- Coarse X increments every 8 pixels
- Coarse Y increments at the end of each visible scanline
- Horizontal bits copy from t to v at cycle 257
- Vertical bits copy from t to v at cycle 280-304 on pre-render scanline

Implement these increments in the `step()` cycle-based logic.

- [ ] **Step 2: Test with SMB — scrolling should work when game starts**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml run -- roms/<smb.nes>`
Expected: the game scrolls smoothly when Mario walks right

- [ ] **Step 3: Commit**

```
git add src/ppu.rs
git commit -m "Implement PPU loopy scroll registers for smooth scrolling"
```

---

## Milestone 6: Joypad Input + Config

### Task 16: Joypad implementation

**Files:**
- Create: `src/joypad.rs`
- Modify: `src/bus.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Implement Joypad struct with strobe protocol**

`src/joypad.rs`:
```rust
#[derive(Debug, Clone, Copy)]
pub enum Button {
    A      = 0,
    B      = 1,
    Select = 2,
    Start  = 3,
    Up     = 4,
    Down   = 5,
    Left   = 6,
    Right  = 7,
}

pub struct Joypad {
    strobe: bool,
    button_index: u8,
    button_state: u8,
}

impl Joypad {
    pub fn new() -> Self {
        Self {
            strobe: false,
            button_index: 0,
            button_state: 0,
        }
    }

    pub fn write(&mut self, data: u8) {
        self.strobe = data & 1 == 1;
        if self.strobe {
            self.button_index = 0;
        }
    }

    pub fn read(&mut self) -> u8 {
        if self.button_index > 7 {
            return 1; // After all 8 buttons, return 1
        }
        let result = (self.button_state >> self.button_index) & 1;
        if !self.strobe {
            self.button_index += 1;
        }
        result
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        if pressed {
            self.button_state |= 1 << button as u8;
        } else {
            self.button_state &= !(1 << button as u8);
        }
    }
}
```

- [ ] **Step 2: Wire joypad into Bus at $4016/$4017**

Update Bus to hold two Joypads, route reads/writes for $4016 (player 1) and $4017 (player 2).

- [ ] **Step 3: Write tests for joypad strobe protocol**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strobe_reset() {
        let mut pad = Joypad::new();
        pad.set_button(Button::A, true);
        pad.set_button(Button::Start, true);
        pad.write(1); // Strobe on
        pad.write(0); // Strobe off — latch state
        assert_eq!(pad.read(), 1); // A
        assert_eq!(pad.read(), 0); // B
        assert_eq!(pad.read(), 0); // Select
        assert_eq!(pad.read(), 1); // Start
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml test`
Expected: all tests pass

- [ ] **Step 5: Commit**

```
git add src/joypad.rs src/bus.rs src/lib.rs
git commit -m "Add Joypad with strobe protocol, wire into Bus"
```

### Task 17: Configuration file

**Files:**
- Create: `src/config.rs`
- Modify: `src/main.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Implement config parsing with defaults**

`src/config.rs`:
```rust
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(default)]
pub struct Config {
    pub display: DisplayConfig,
    pub rom: RomConfig,
    pub input: InputConfig,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub scale: u32,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct RomConfig {
    pub path: String,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct InputConfig {
    pub player1: PlayerInput,
    pub player2: PlayerInput,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct PlayerInput {
    pub up: String,
    pub down: String,
    pub left: String,
    pub right: String,
    pub a: String,
    pub b: String,
    pub select: String,
    pub start: String,
}

// Default implementations
impl Default for Config {
    fn default() -> Self {
        Self {
            display: DisplayConfig::default(),
            rom: RomConfig::default(),
            input: InputConfig::default(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self { scale: 3 }
    }
}

impl Default for RomConfig {
    fn default() -> Self {
        Self { path: "./roms".into() }
    }
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            player1: PlayerInput::default_p1(),
            player2: PlayerInput::default_p2(),
        }
    }
}

impl PlayerInput {
    fn default_p1() -> Self {
        Self {
            up: "E".into(),
            down: "D".into(),
            left: "S".into(),
            right: "F".into(),
            a: "K".into(),
            b: "J".into(),
            select: "G".into(),
            start: "H".into(),
        }
    }

    fn default_p2() -> Self {
        Self {
            up: "Up".into(),
            down: "Down".into(),
            left: "Left".into(),
            right: "Right".into(),
            a: "Numpad2".into(),
            b: "Numpad1".into(),
            select: "Numpad5".into(),
            start: "Numpad6".into(),
        }
    }
}

impl Default for PlayerInput {
    fn default() -> Self {
        Self::default_p1()
    }
}

impl Config {
    pub fn load() -> Self {
        // Try ./rfc.toml first, then ~/.config/rfc/rfc.toml
        let paths = [
            PathBuf::from("rfc.toml"),
            dirs::config_dir()
                .map(|d| d.join("rfc").join("rfc.toml"))
                .unwrap_or_default(),
        ];

        for path in &paths {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Ok(config) = toml::from_str(&content) {
                        log::info!("Loaded config from {}", path.display());
                        return config;
                    }
                }
            }
        }

        log::info!("Using default config");
        Config::default()
    }
}
```

- [ ] **Step 2: Map config key names to winit KeyCode in main.rs**

Add a function to convert string key names (from config) to `winit::keyboard::KeyCode`. Wire keyboard events to joypad button state via Console.

- [ ] **Step 3: Write tests for config parsing**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.display.scale, 3);
        assert_eq!(config.input.player1.up, "E");
        assert_eq!(config.input.player1.select, "G");
        assert_eq!(config.input.player1.start, "H");
    }

    #[test]
    fn test_parse_partial_toml() {
        let toml_str = r#"
            [display]
            scale = 4
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.display.scale, 4);
        // Other values should be defaults
        assert_eq!(config.input.player1.up, "E");
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml test`
Expected: all tests pass

- [ ] **Step 5: Update main.rs to use config**

Load config at startup, use `config.display.scale` for window size, use key mappings for joypad input.

- [ ] **Step 6: Test with SMB — game should be playable**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml run -- roms/<smb.nes>`
Expected: SMB is playable with ESDF + JK + GH keys

- [ ] **Step 7: Commit**

```
git add src/config.rs src/main.rs src/lib.rs
git commit -m "Add TOML config with key mapping, SMB playable"
```

---

## Milestone 7: Mapper 2 (Contra)

### Task 18: Mapper 2 (UxROM) implementation

**Files:**
- Create: `src/mapper/mapper2.rs`
- Modify: `src/mapper/mod.rs`
- Modify: `src/cartridge.rs`

- [ ] **Step 1: Implement Mapper 2**

`src/mapper/mapper2.rs`:
```rust
use super::Mapper;

/// UxROM — switchable 16KB PRG bank + fixed last bank
/// CHR RAM (8KB, writable)
pub struct Mapper2 {
    prg_rom: Vec<u8>,
    chr_ram: Vec<u8>,
    bank_select: u8,
    prg_banks: u8, // Total number of 16KB banks
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
                // Switchable bank
                let offset = self.bank_select as usize * 16384 + (addr - 0x8000) as usize;
                self.prg_rom[offset]
            }
            0xC000..=0xFFFF => {
                // Fixed to last bank
                let offset = (self.prg_banks as usize - 1) * 16384 + (addr - 0xC000) as usize;
                self.prg_rom[offset]
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, data: u8) {
        match addr {
            0x8000..=0xFFFF => {
                self.bank_select = data & 0x0F; // Lower 4 bits select bank
            }
            _ => {}
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.chr_ram[addr as usize],
            _ => 0,
        }
    }

    fn ppu_write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => self.chr_ram[addr as usize] = data, // CHR RAM is writable
            _ => {}
        }
    }
}
```

- [ ] **Step 2: Register Mapper 2 in cartridge factory**

Update `src/cartridge.rs` to handle mapper number 2:
```rust
use crate::mapper::mapper2::Mapper2;

// In from_ines():
let mapper: Box<dyn Mapper> = match mapper_number {
    0 => Box::new(Mapper0::new(prg_rom, chr_rom)),
    2 => Box::new(Mapper2::new(prg_rom, chr_rom)),
    _ => return Err(format!("Unsupported mapper: {}", mapper_number)),
};
```

Update `src/mapper/mod.rs`:
```rust
pub mod mapper0;
pub mod mapper2;
```

- [ ] **Step 3: Write tests for Mapper 2**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bank_switching() {
        let mut prg = vec![0u8; 16384 * 4]; // 4 banks
        prg[0] = 0xAA;           // Bank 0, first byte
        prg[16384] = 0xBB;       // Bank 1, first byte
        let mut mapper = Mapper2::new(prg, vec![]);

        assert_eq!(mapper.cpu_read(0x8000), 0xAA); // Default bank 0

        mapper.cpu_write(0x8000, 1); // Switch to bank 1
        assert_eq!(mapper.cpu_read(0x8000), 0xBB);
    }

    #[test]
    fn test_fixed_last_bank() {
        let mut prg = vec![0u8; 16384 * 4];
        prg[16384 * 3] = 0xCC; // Last bank, first byte
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
```

- [ ] **Step 4: Run tests**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml test`
Expected: all tests pass

- [ ] **Step 5: Test with Contra ROM**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml run -- roms/<contra.nes>`
Expected: Contra loads and is playable

- [ ] **Step 6: Commit**

```
git add src/mapper/mapper2.rs src/mapper/mod.rs src/cartridge.rs
git commit -m "Add Mapper 2 (UxROM) — Contra playable"
```

---

## Milestone 8: APU (Audio)

### Task 19: APU struct and register interface

**Files:**
- Create: `src/apu.rs`
- Modify: `src/bus.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Implement APU with pulse, triangle, and noise channels**

`src/apu.rs` — implement:
- Two pulse wave channels ($4000-$4007)
- Triangle wave channel ($4008-$400B)
- Noise channel ($400C-$400F)
- Status register ($4015)
- Frame counter ($4017)
- Audio sample output mixed from all channels

Each channel has: timer, length counter, envelope (pulse/noise), sweep (pulse), linear counter (triangle).

The APU generates samples at CPU clock / N. A ring buffer collects samples, which cpal reads from its audio callback.

- [ ] **Step 2: Wire APU into Bus**

Route $4000-$4013, $4015, $4017 to APU in Bus read/write.

- [ ] **Step 3: Wire cpal audio output in main.rs**

Set up cpal output stream. APU fills a shared ring buffer, cpal callback reads from it.

- [ ] **Step 4: Test with SMB — should hear music and sound effects**

Run: `cargo --manifest-path /Users/rainux/devel/live/rust/rfc/Cargo.toml run -- roms/<smb.nes>`
Expected: SMB plays with audio

- [ ] **Step 5: Commit**

```
git add src/apu.rs src/bus.rs src/lib.rs src/main.rs
git commit -m "Add APU with pulse, triangle, and noise channels — audio output working"
```

---

## Summary

| Task | Milestone | Description |
|------|-----------|-------------|
| 1 | 1 | Project setup and dependencies |
| 2 | 1 | Bus — address routing skeleton |
| 3 | 1 | CPU — registers and basic infrastructure |
| 4 | 1 | CPU — addressing modes |
| 5 | 1 | CPU — opcode table and step() |
| 6 | 2 | Mapper trait and Mapper 0 |
| 7 | 2 | Cartridge — iNES file parser |
| 8 | 2 | Wire Cartridge into Bus |
| 9 | 2 | Download nestest and verify loading |
| 10 | 3 | nestest log comparison harness — pass all tests |
| 11 | 4 | PPU struct and register I/O |
| 12 | 4 | PPU scanline rendering engine |
| 13 | 4 | Console and main loop — render a frame |
| 14 | 4 | PPU sprite rendering and sprite-0 hit |
| 15 | 5 | PPU scroll register mechanics (loopy) |
| 16 | 6 | Joypad implementation |
| 17 | 6 | Configuration file |
| 18 | 7 | Mapper 2 (UxROM) — Contra |
| 19 | 8 | APU audio output |
