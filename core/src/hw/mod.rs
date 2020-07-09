mod header;
pub mod mmu;
mod scheduler;

use header::Header;
pub use mmu::{AccessType, MemoryValue};
use scheduler::Scheduler;

pub struct HW {
    bios7: Vec<u8>,
    bios9: Vec<u8>,
    rom_header: Header,
    rom: Vec<u8>,
    main_mem: Vec<u8>,
    iwram: Vec<u8>,

    scheduler: Scheduler,
}

impl HW {
    const MAIN_MEM_SIZE: usize = 0x20_0000;
    const IWRAM_SIZE: usize = 0x1_0000;

    pub fn new(bios7: Vec<u8>, bios9: Vec<u8>, rom: Vec<u8>) -> Self {
        let mut main_mem = vec![0; HW::MAIN_MEM_SIZE];
        let rom_header = Header::new(&rom);
        let addr = 0x027F_FE00 & (HW::MAIN_MEM_SIZE - 1); 
        main_mem[addr..addr + 0x160].copy_from_slice(&rom[..0x160]);
        HW {
            bios7,
            bios9,
            rom_header,
            rom,
            main_mem,
            iwram: vec![0; HW::IWRAM_SIZE],

            scheduler: Scheduler::new(),
        }
    }

    pub fn clock(&mut self, cycles: usize) {
        for _ in 0..cycles {
            self.handle_events();
        }
    }

    pub fn interrupts_requested(&self) -> bool {
        false
    }
}
