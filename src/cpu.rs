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
}

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
