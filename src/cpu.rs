use crate::bus::Bus;

#[derive(Debug, Clone, Copy, PartialEq)]
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
        self.cycles = 7;
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
        let hi_addr = (addr & 0xFF00) | ((addr.wrapping_add(1)) & 0x00FF);
        let hi = bus.read(hi_addr) as u16;
        (hi << 8) | lo
    }

    /// Internal ADC logic: A + operand + C -> A, sets C, Z, V, N
    fn adc(&mut self, operand: u8) {
        let a = self.a as u16;
        let m = operand as u16;
        let c = if self.get_flag(CARRY) { 1u16 } else { 0u16 };
        let sum = a + m + c;
        let result = sum as u8;

        self.set_flag(CARRY, sum > 0xFF);
        self.set_flag(ZERO, result == 0);
        self.set_flag(OVERFLOW, (!(a ^ m) & (a ^ sum)) & 0x80 != 0);
        self.set_flag(NEGATIVE, result & 0x80 != 0);
        self.a = result;
    }

    /// Compare: reg - operand, set C, Z, N
    fn compare(&mut self, reg: u8, operand: u8) {
        let result = reg.wrapping_sub(operand);
        self.set_flag(CARRY, reg >= operand);
        self.set_flag(ZERO, reg == operand);
        self.set_flag(NEGATIVE, result & 0x80 != 0);
    }

    /// Branch helper: returns extra cycles (1 if taken, +1 if page cross)
    fn branch(&mut self, bus: &mut Bus, condition: bool) -> u8 {
        let offset = self.fetch(bus) as i8;
        if condition {
            let old_pc = self.pc;
            self.pc = self.pc.wrapping_add(offset as u16);
            let mut extra = 1;
            if (old_pc & 0xFF00) != (self.pc & 0xFF00) {
                extra += 1;
            }
            extra
        } else {
            0
        }
    }

    /// Execute one instruction, return cycles consumed
    pub fn step(&mut self, bus: &mut Bus) -> u8 {
        let opcode = self.fetch(bus);

        let cycles: u8 = match opcode {
            // === Load/Store ===

            // LDA
            0xA9 => { let (addr, _) = self.resolve_address(bus, AddressingMode::Immediate); self.a = bus.read(addr); self.update_zero_and_negative(self.a); 2 }
            0xA5 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); self.a = bus.read(addr); self.update_zero_and_negative(self.a); 3 }
            0xB5 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); self.a = bus.read(addr); self.update_zero_and_negative(self.a); 4 }
            0xAD => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); self.a = bus.read(addr); self.update_zero_and_negative(self.a); 4 }
            0xBD => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteX); self.a = bus.read(addr); self.update_zero_and_negative(self.a); 4 + pc as u8 }
            0xB9 => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteY); self.a = bus.read(addr); self.update_zero_and_negative(self.a); 4 + pc as u8 }
            0xA1 => { let (addr, _) = self.resolve_address(bus, AddressingMode::IndirectX); self.a = bus.read(addr); self.update_zero_and_negative(self.a); 6 }
            0xB1 => { let (addr, pc) = self.resolve_address(bus, AddressingMode::IndirectY); self.a = bus.read(addr); self.update_zero_and_negative(self.a); 5 + pc as u8 }

            // LDX
            0xA2 => { let (addr, _) = self.resolve_address(bus, AddressingMode::Immediate); self.x = bus.read(addr); self.update_zero_and_negative(self.x); 2 }
            0xA6 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); self.x = bus.read(addr); self.update_zero_and_negative(self.x); 3 }
            0xB6 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageY); self.x = bus.read(addr); self.update_zero_and_negative(self.x); 4 }
            0xAE => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); self.x = bus.read(addr); self.update_zero_and_negative(self.x); 4 }
            0xBE => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteY); self.x = bus.read(addr); self.update_zero_and_negative(self.x); 4 + pc as u8 }

            // LDY
            0xA0 => { let (addr, _) = self.resolve_address(bus, AddressingMode::Immediate); self.y = bus.read(addr); self.update_zero_and_negative(self.y); 2 }
            0xA4 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); self.y = bus.read(addr); self.update_zero_and_negative(self.y); 3 }
            0xB4 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); self.y = bus.read(addr); self.update_zero_and_negative(self.y); 4 }
            0xAC => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); self.y = bus.read(addr); self.update_zero_and_negative(self.y); 4 }
            0xBC => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteX); self.y = bus.read(addr); self.update_zero_and_negative(self.y); 4 + pc as u8 }

            // STA
            0x85 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); bus.write(addr, self.a); 3 }
            0x95 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); bus.write(addr, self.a); 4 }
            0x8D => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); bus.write(addr, self.a); 4 }
            0x9D => { let (addr, _) = self.resolve_address(bus, AddressingMode::AbsoluteX); bus.write(addr, self.a); 5 }
            0x99 => { let (addr, _) = self.resolve_address(bus, AddressingMode::AbsoluteY); bus.write(addr, self.a); 5 }
            0x81 => { let (addr, _) = self.resolve_address(bus, AddressingMode::IndirectX); bus.write(addr, self.a); 6 }
            0x91 => { let (addr, _) = self.resolve_address(bus, AddressingMode::IndirectY); bus.write(addr, self.a); 6 }

            // STX
            0x86 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); bus.write(addr, self.x); 3 }
            0x96 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageY); bus.write(addr, self.x); 4 }
            0x8E => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); bus.write(addr, self.x); 4 }

            // STY
            0x84 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); bus.write(addr, self.y); 3 }
            0x94 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); bus.write(addr, self.y); 4 }
            0x8C => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); bus.write(addr, self.y); 4 }

            // === Arithmetic ===

            // ADC
            0x69 => { let (addr, _) = self.resolve_address(bus, AddressingMode::Immediate); let m = bus.read(addr); self.adc(m); 2 }
            0x65 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); let m = bus.read(addr); self.adc(m); 3 }
            0x75 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); let m = bus.read(addr); self.adc(m); 4 }
            0x6D => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); let m = bus.read(addr); self.adc(m); 4 }
            0x7D => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteX); let m = bus.read(addr); self.adc(m); 4 + pc as u8 }
            0x79 => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteY); let m = bus.read(addr); self.adc(m); 4 + pc as u8 }
            0x61 => { let (addr, _) = self.resolve_address(bus, AddressingMode::IndirectX); let m = bus.read(addr); self.adc(m); 6 }
            0x71 => { let (addr, pc) = self.resolve_address(bus, AddressingMode::IndirectY); let m = bus.read(addr); self.adc(m); 5 + pc as u8 }

            // SBC
            0xE9 => { let (addr, _) = self.resolve_address(bus, AddressingMode::Immediate); let m = bus.read(addr); self.adc(!m); 2 }
            0xE5 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); let m = bus.read(addr); self.adc(!m); 3 }
            0xF5 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); let m = bus.read(addr); self.adc(!m); 4 }
            0xED => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); let m = bus.read(addr); self.adc(!m); 4 }
            0xFD => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteX); let m = bus.read(addr); self.adc(!m); 4 + pc as u8 }
            0xF9 => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteY); let m = bus.read(addr); self.adc(!m); 4 + pc as u8 }
            0xE1 => { let (addr, _) = self.resolve_address(bus, AddressingMode::IndirectX); let m = bus.read(addr); self.adc(!m); 6 }
            0xF1 => { let (addr, pc) = self.resolve_address(bus, AddressingMode::IndirectY); let m = bus.read(addr); self.adc(!m); 5 + pc as u8 }

            // === Compare ===

            // CMP
            0xC9 => { let (addr, _) = self.resolve_address(bus, AddressingMode::Immediate); let m = bus.read(addr); self.compare(self.a, m); 2 }
            0xC5 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); let m = bus.read(addr); self.compare(self.a, m); 3 }
            0xD5 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); let m = bus.read(addr); self.compare(self.a, m); 4 }
            0xCD => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); let m = bus.read(addr); self.compare(self.a, m); 4 }
            0xDD => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteX); let m = bus.read(addr); self.compare(self.a, m); 4 + pc as u8 }
            0xD9 => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteY); let m = bus.read(addr); self.compare(self.a, m); 4 + pc as u8 }
            0xC1 => { let (addr, _) = self.resolve_address(bus, AddressingMode::IndirectX); let m = bus.read(addr); self.compare(self.a, m); 6 }
            0xD1 => { let (addr, pc) = self.resolve_address(bus, AddressingMode::IndirectY); let m = bus.read(addr); self.compare(self.a, m); 5 + pc as u8 }

            // CPX
            0xE0 => { let (addr, _) = self.resolve_address(bus, AddressingMode::Immediate); let m = bus.read(addr); self.compare(self.x, m); 2 }
            0xE4 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); let m = bus.read(addr); self.compare(self.x, m); 3 }
            0xEC => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); let m = bus.read(addr); self.compare(self.x, m); 4 }

            // CPY
            0xC0 => { let (addr, _) = self.resolve_address(bus, AddressingMode::Immediate); let m = bus.read(addr); self.compare(self.y, m); 2 }
            0xC4 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); let m = bus.read(addr); self.compare(self.y, m); 3 }
            0xCC => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); let m = bus.read(addr); self.compare(self.y, m); 4 }

            // === Logic ===

            // AND
            0x29 => { let (addr, _) = self.resolve_address(bus, AddressingMode::Immediate); self.a &= bus.read(addr); self.update_zero_and_negative(self.a); 2 }
            0x25 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); self.a &= bus.read(addr); self.update_zero_and_negative(self.a); 3 }
            0x35 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); self.a &= bus.read(addr); self.update_zero_and_negative(self.a); 4 }
            0x2D => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); self.a &= bus.read(addr); self.update_zero_and_negative(self.a); 4 }
            0x3D => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteX); self.a &= bus.read(addr); self.update_zero_and_negative(self.a); 4 + pc as u8 }
            0x39 => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteY); self.a &= bus.read(addr); self.update_zero_and_negative(self.a); 4 + pc as u8 }
            0x21 => { let (addr, _) = self.resolve_address(bus, AddressingMode::IndirectX); self.a &= bus.read(addr); self.update_zero_and_negative(self.a); 6 }
            0x31 => { let (addr, pc) = self.resolve_address(bus, AddressingMode::IndirectY); self.a &= bus.read(addr); self.update_zero_and_negative(self.a); 5 + pc as u8 }

            // ORA
            0x09 => { let (addr, _) = self.resolve_address(bus, AddressingMode::Immediate); self.a |= bus.read(addr); self.update_zero_and_negative(self.a); 2 }
            0x05 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); self.a |= bus.read(addr); self.update_zero_and_negative(self.a); 3 }
            0x15 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); self.a |= bus.read(addr); self.update_zero_and_negative(self.a); 4 }
            0x0D => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); self.a |= bus.read(addr); self.update_zero_and_negative(self.a); 4 }
            0x1D => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteX); self.a |= bus.read(addr); self.update_zero_and_negative(self.a); 4 + pc as u8 }
            0x19 => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteY); self.a |= bus.read(addr); self.update_zero_and_negative(self.a); 4 + pc as u8 }
            0x01 => { let (addr, _) = self.resolve_address(bus, AddressingMode::IndirectX); self.a |= bus.read(addr); self.update_zero_and_negative(self.a); 6 }
            0x11 => { let (addr, pc) = self.resolve_address(bus, AddressingMode::IndirectY); self.a |= bus.read(addr); self.update_zero_and_negative(self.a); 5 + pc as u8 }

            // EOR
            0x49 => { let (addr, _) = self.resolve_address(bus, AddressingMode::Immediate); self.a ^= bus.read(addr); self.update_zero_and_negative(self.a); 2 }
            0x45 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); self.a ^= bus.read(addr); self.update_zero_and_negative(self.a); 3 }
            0x55 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); self.a ^= bus.read(addr); self.update_zero_and_negative(self.a); 4 }
            0x4D => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); self.a ^= bus.read(addr); self.update_zero_and_negative(self.a); 4 }
            0x5D => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteX); self.a ^= bus.read(addr); self.update_zero_and_negative(self.a); 4 + pc as u8 }
            0x59 => { let (addr, pc) = self.resolve_address(bus, AddressingMode::AbsoluteY); self.a ^= bus.read(addr); self.update_zero_and_negative(self.a); 4 + pc as u8 }
            0x41 => { let (addr, _) = self.resolve_address(bus, AddressingMode::IndirectX); self.a ^= bus.read(addr); self.update_zero_and_negative(self.a); 6 }
            0x51 => { let (addr, pc) = self.resolve_address(bus, AddressingMode::IndirectY); self.a ^= bus.read(addr); self.update_zero_and_negative(self.a); 5 + pc as u8 }

            // BIT
            0x24 => {
                let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage);
                let m = bus.read(addr);
                self.set_flag(ZERO, self.a & m == 0);
                self.set_flag(NEGATIVE, m & 0x80 != 0);
                self.set_flag(OVERFLOW, m & 0x40 != 0);
                3
            }
            0x2C => {
                let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute);
                let m = bus.read(addr);
                self.set_flag(ZERO, self.a & m == 0);
                self.set_flag(NEGATIVE, m & 0x80 != 0);
                self.set_flag(OVERFLOW, m & 0x40 != 0);
                4
            }

            // === Shifts/Rotates ===

            // ASL
            0x0A => {
                self.set_flag(CARRY, self.a & 0x80 != 0);
                self.a <<= 1;
                self.update_zero_and_negative(self.a);
                2
            }
            0x06 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); let mut m = bus.read(addr); self.set_flag(CARRY, m & 0x80 != 0); m <<= 1; bus.write(addr, m); self.update_zero_and_negative(m); 5 }
            0x16 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); let mut m = bus.read(addr); self.set_flag(CARRY, m & 0x80 != 0); m <<= 1; bus.write(addr, m); self.update_zero_and_negative(m); 6 }
            0x0E => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); let mut m = bus.read(addr); self.set_flag(CARRY, m & 0x80 != 0); m <<= 1; bus.write(addr, m); self.update_zero_and_negative(m); 6 }
            0x1E => { let (addr, _) = self.resolve_address(bus, AddressingMode::AbsoluteX); let mut m = bus.read(addr); self.set_flag(CARRY, m & 0x80 != 0); m <<= 1; bus.write(addr, m); self.update_zero_and_negative(m); 7 }

            // LSR
            0x4A => {
                self.set_flag(CARRY, self.a & 0x01 != 0);
                self.a >>= 1;
                self.update_zero_and_negative(self.a);
                2
            }
            0x46 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); let mut m = bus.read(addr); self.set_flag(CARRY, m & 0x01 != 0); m >>= 1; bus.write(addr, m); self.update_zero_and_negative(m); 5 }
            0x56 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); let mut m = bus.read(addr); self.set_flag(CARRY, m & 0x01 != 0); m >>= 1; bus.write(addr, m); self.update_zero_and_negative(m); 6 }
            0x4E => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); let mut m = bus.read(addr); self.set_flag(CARRY, m & 0x01 != 0); m >>= 1; bus.write(addr, m); self.update_zero_and_negative(m); 6 }
            0x5E => { let (addr, _) = self.resolve_address(bus, AddressingMode::AbsoluteX); let mut m = bus.read(addr); self.set_flag(CARRY, m & 0x01 != 0); m >>= 1; bus.write(addr, m); self.update_zero_and_negative(m); 7 }

            // ROL
            0x2A => {
                let old_carry = self.get_flag(CARRY) as u8;
                self.set_flag(CARRY, self.a & 0x80 != 0);
                self.a = (self.a << 1) | old_carry;
                self.update_zero_and_negative(self.a);
                2
            }
            0x26 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); let mut m = bus.read(addr); let old_carry = self.get_flag(CARRY) as u8; self.set_flag(CARRY, m & 0x80 != 0); m = (m << 1) | old_carry; bus.write(addr, m); self.update_zero_and_negative(m); 5 }
            0x36 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); let mut m = bus.read(addr); let old_carry = self.get_flag(CARRY) as u8; self.set_flag(CARRY, m & 0x80 != 0); m = (m << 1) | old_carry; bus.write(addr, m); self.update_zero_and_negative(m); 6 }
            0x2E => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); let mut m = bus.read(addr); let old_carry = self.get_flag(CARRY) as u8; self.set_flag(CARRY, m & 0x80 != 0); m = (m << 1) | old_carry; bus.write(addr, m); self.update_zero_and_negative(m); 6 }
            0x3E => { let (addr, _) = self.resolve_address(bus, AddressingMode::AbsoluteX); let mut m = bus.read(addr); let old_carry = self.get_flag(CARRY) as u8; self.set_flag(CARRY, m & 0x80 != 0); m = (m << 1) | old_carry; bus.write(addr, m); self.update_zero_and_negative(m); 7 }

            // ROR
            0x6A => {
                let old_carry = self.get_flag(CARRY) as u8;
                self.set_flag(CARRY, self.a & 0x01 != 0);
                self.a = (self.a >> 1) | (old_carry << 7);
                self.update_zero_and_negative(self.a);
                2
            }
            0x66 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); let mut m = bus.read(addr); let old_carry = self.get_flag(CARRY) as u8; self.set_flag(CARRY, m & 0x01 != 0); m = (m >> 1) | (old_carry << 7); bus.write(addr, m); self.update_zero_and_negative(m); 5 }
            0x76 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); let mut m = bus.read(addr); let old_carry = self.get_flag(CARRY) as u8; self.set_flag(CARRY, m & 0x01 != 0); m = (m >> 1) | (old_carry << 7); bus.write(addr, m); self.update_zero_and_negative(m); 6 }
            0x6E => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); let mut m = bus.read(addr); let old_carry = self.get_flag(CARRY) as u8; self.set_flag(CARRY, m & 0x01 != 0); m = (m >> 1) | (old_carry << 7); bus.write(addr, m); self.update_zero_and_negative(m); 6 }
            0x7E => { let (addr, _) = self.resolve_address(bus, AddressingMode::AbsoluteX); let mut m = bus.read(addr); let old_carry = self.get_flag(CARRY) as u8; self.set_flag(CARRY, m & 0x01 != 0); m = (m >> 1) | (old_carry << 7); bus.write(addr, m); self.update_zero_and_negative(m); 7 }

            // === Inc/Dec ===

            // INC
            0xE6 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); let m = bus.read(addr).wrapping_add(1); bus.write(addr, m); self.update_zero_and_negative(m); 5 }
            0xF6 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); let m = bus.read(addr).wrapping_add(1); bus.write(addr, m); self.update_zero_and_negative(m); 6 }
            0xEE => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); let m = bus.read(addr).wrapping_add(1); bus.write(addr, m); self.update_zero_and_negative(m); 6 }
            0xFE => { let (addr, _) = self.resolve_address(bus, AddressingMode::AbsoluteX); let m = bus.read(addr).wrapping_add(1); bus.write(addr, m); self.update_zero_and_negative(m); 7 }

            // DEC
            0xC6 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPage); let m = bus.read(addr).wrapping_sub(1); bus.write(addr, m); self.update_zero_and_negative(m); 5 }
            0xD6 => { let (addr, _) = self.resolve_address(bus, AddressingMode::ZeroPageX); let m = bus.read(addr).wrapping_sub(1); bus.write(addr, m); self.update_zero_and_negative(m); 6 }
            0xCE => { let (addr, _) = self.resolve_address(bus, AddressingMode::Absolute); let m = bus.read(addr).wrapping_sub(1); bus.write(addr, m); self.update_zero_and_negative(m); 6 }
            0xDE => { let (addr, _) = self.resolve_address(bus, AddressingMode::AbsoluteX); let m = bus.read(addr).wrapping_sub(1); bus.write(addr, m); self.update_zero_and_negative(m); 7 }

            // INX
            0xE8 => { self.x = self.x.wrapping_add(1); self.update_zero_and_negative(self.x); 2 }
            // INY
            0xC8 => { self.y = self.y.wrapping_add(1); self.update_zero_and_negative(self.y); 2 }
            // DEX
            0xCA => { self.x = self.x.wrapping_sub(1); self.update_zero_and_negative(self.x); 2 }
            // DEY
            0x88 => { self.y = self.y.wrapping_sub(1); self.update_zero_and_negative(self.y); 2 }

            // === Transfers ===
            0xAA => { self.x = self.a; self.update_zero_and_negative(self.x); 2 } // TAX
            0xA8 => { self.y = self.a; self.update_zero_and_negative(self.y); 2 } // TAY
            0x8A => { self.a = self.x; self.update_zero_and_negative(self.a); 2 } // TXA
            0x98 => { self.a = self.y; self.update_zero_and_negative(self.a); 2 } // TYA
            0xBA => { self.x = self.sp; self.update_zero_and_negative(self.x); 2 } // TSX
            0x9A => { self.sp = self.x; 2 } // TXS (no flags)

            // === Branch ===
            0x90 => { 2 + self.branch(bus, !self.get_flag(CARRY)) }   // BCC
            0xB0 => { 2 + self.branch(bus, self.get_flag(CARRY)) }    // BCS
            0xF0 => { 2 + self.branch(bus, self.get_flag(ZERO)) }     // BEQ
            0xD0 => { 2 + self.branch(bus, !self.get_flag(ZERO)) }    // BNE
            0x30 => { 2 + self.branch(bus, self.get_flag(NEGATIVE)) }  // BMI
            0x10 => { 2 + self.branch(bus, !self.get_flag(NEGATIVE)) } // BPL
            0x50 => { 2 + self.branch(bus, !self.get_flag(OVERFLOW)) } // BVC
            0x70 => { 2 + self.branch(bus, self.get_flag(OVERFLOW)) }  // BVS

            // === Jump/Call ===

            // JMP absolute
            0x4C => {
                let addr = self.fetch16(bus);
                self.pc = addr;
                3
            }
            // JMP indirect
            0x6C => {
                let ptr = self.fetch16(bus);
                self.pc = self.read16_wrap(bus, ptr);
                5
            }
            // JSR
            0x20 => {
                let addr = self.fetch16(bus);
                // Push PC-1 (address of last byte of JSR instruction)
                self.push16(bus, self.pc.wrapping_sub(1));
                self.pc = addr;
                6
            }
            // RTS
            0x60 => {
                let addr = self.pull16(bus);
                self.pc = addr.wrapping_add(1);
                6
            }
            // RTI
            0x40 => {
                let status = self.pull(bus);
                self.status = (status & !BREAK) | UNUSED;
                self.pc = self.pull16(bus);
                6
            }

            // === Stack ===

            // PHA
            0x48 => { self.push(bus, self.a); 3 }
            // PHP
            0x08 => { self.push(bus, self.status | BREAK | UNUSED); 3 }
            // PLA
            0x68 => { self.a = self.pull(bus); self.update_zero_and_negative(self.a); 4 }
            // PLP
            0x28 => { let val = self.pull(bus); self.status = (val & !BREAK) | UNUSED; 4 }

            // === Flag ===

            0x18 => { self.set_flag(CARRY, false); 2 }            // CLC
            0x38 => { self.set_flag(CARRY, true); 2 }             // SEC
            0x58 => { self.set_flag(INTERRUPT_DISABLE, false); 2 } // CLI
            0x78 => { self.set_flag(INTERRUPT_DISABLE, true); 2 }  // SEI
            0xD8 => { self.set_flag(DECIMAL, false); 2 }          // CLD
            0xF8 => { self.set_flag(DECIMAL, true); 2 }           // SED
            0xB8 => { self.set_flag(OVERFLOW, false); 2 }         // CLV

            // === System ===

            // BRK
            0x00 => {
                self.pc = self.pc.wrapping_add(1); // Skip padding byte (PC+2 total)
                self.push16(bus, self.pc);
                self.push(bus, self.status | BREAK | UNUSED);
                self.set_flag(INTERRUPT_DISABLE, true);
                let lo = bus.read(0xFFFE) as u16;
                let hi = bus.read(0xFFFF) as u16;
                self.pc = (hi << 8) | lo;
                7
            }
            // NOP
            0xEA => { 2 }

            // Unofficial/undefined opcodes - treat as NOP
            _ => { 2 }
        };

        self.cycles += cycles as u64;
        cycles
    }

    pub fn nmi(&mut self, bus: &mut Bus) {
        self.push16(bus, self.pc);
        self.push(bus, (self.status | UNUSED) & !BREAK);
        self.set_flag(INTERRUPT_DISABLE, true);
        let lo = bus.read(0xFFFA) as u16;
        let hi = bus.read(0xFFFB) as u16;
        self.pc = (hi << 8) | lo;
        self.cycles += 7;
    }

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

    /// Resolve addressing mode to (effective_address, page_crossed)
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
                (addr, false)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_cpu_with_ram(bytes: &[(u16, u8)]) -> (Cpu, Bus) {
        let cpu = Cpu::new();
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
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0x10),
            (0x0010, 0x00),
            (0x0011, 0x03),
        ]);
        cpu.pc = 0x0000;
        cpu.y = 0x05;
        let (addr, _) = cpu.resolve_address(&mut bus, AddressingMode::IndirectY);
        assert_eq!(addr, 0x0305);
    }

    #[test]
    fn test_indirect_x() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0x20),
            (0x0024, 0x00),
            (0x0025, 0x03),
        ]);
        cpu.pc = 0x0000;
        cpu.x = 0x04;
        let (addr, _) = cpu.resolve_address(&mut bus, AddressingMode::IndirectX);
        assert_eq!(addr, 0x0300);
    }

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

    // === step() instruction tests ===

    #[test]
    fn test_lda_immediate() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0xA9), (0x0001, 0x42),
        ]);
        cpu.pc = 0x0000;
        let cycles = cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x42);
        assert_eq!(cycles, 2);
    }

    #[test]
    fn test_lda_zero_flag() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0xA9), (0x0001, 0x00),
        ]);
        cpu.pc = 0x0000;
        cpu.step(&mut bus);
        assert!(cpu.get_flag(ZERO));
    }

    #[test]
    fn test_sta_zero_page() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0x85), (0x0001, 0x10),
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
            (0x0000, 0x4C), (0x0001, 0x00), (0x0002, 0x06),
        ]);
        cpu.pc = 0x0000;
        cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x0600);
    }

    #[test]
    fn test_adc_with_carry() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0x69), (0x0001, 0x01),
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
            (0x0000, 0x69), (0x0001, 0x50),
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
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[(0x0000, 0xE8)]);
        cpu.pc = 0x0000;
        cpu.x = 0xFF;
        cpu.step(&mut bus);
        assert_eq!(cpu.x, 0x00);
        assert!(cpu.get_flag(ZERO));
    }

    #[test]
    fn test_bne_taken() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0xD0), (0x0001, 0x02),
        ]);
        cpu.pc = 0x0000;
        cpu.set_flag(ZERO, false);
        let cycles = cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x0004);
        assert_eq!(cycles, 3);
    }

    #[test]
    fn test_bne_not_taken() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0xD0), (0x0001, 0x02),
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
            (0x0000, 0x20), (0x0001, 0x00), (0x0002, 0x06),
            (0x0600, 0x60),
        ]);
        cpu.pc = 0x0000;
        cpu.sp = 0xFF;
        cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x0600);
        cpu.step(&mut bus);
        assert_eq!(cpu.pc, 0x0003);
    }

    #[test]
    fn test_pha_pla() {
        let (mut cpu, mut bus) = setup_cpu_with_ram(&[
            (0x0000, 0x48), // PHA
            (0x0001, 0xA9), (0x0002, 0x00), // LDA #0
            (0x0003, 0x68), // PLA
        ]);
        cpu.pc = 0x0000;
        cpu.sp = 0xFF;
        cpu.a = 0x42;
        cpu.step(&mut bus);
        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x00);
        cpu.step(&mut bus);
        assert_eq!(cpu.a, 0x42);
    }
}
