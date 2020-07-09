mod header;
pub mod mmu;

use header::Header;
pub use mmu::{AccessType, MemoryValue};

pub struct HW {
    bios7: Vec<u8>,
    bios9: Vec<u8>,
    rom_header: Header,
    rom: Vec<u8>,
}

impl HW {
    pub fn new(bios7: Vec<u8>, bios9: Vec<u8>, rom: Vec<u8>) -> Self {
        HW {
            bios7,
            bios9,
            rom_header: Header::new(&rom),
            rom,
        }
    }

    pub fn interrupts_requested(&self) -> bool {
        false
    }
}
