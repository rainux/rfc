pub mod mapper0;
pub mod mapper2;
pub mod mapper3;
pub mod mapper4;

use crate::cartridge::Mirroring;

pub trait Mapper {
    fn cpu_read(&self, addr: u16) -> u8;
    fn cpu_write(&mut self, addr: u16, data: u8);
    fn ppu_read(&self, addr: u16) -> u8;
    fn ppu_write(&mut self, addr: u16, data: u8);
    fn notify_scanline(&mut self) {}
    fn irq_pending(&self) -> bool {
        false
    }
    fn irq_acknowledge(&mut self) {}
    /// Return current mirroring if the mapper controls it dynamically
    fn mirroring(&self) -> Option<Mirroring> {
        None
    }
}
