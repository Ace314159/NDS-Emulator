mod header;
pub mod mmu;
mod scheduler;
mod interrupt_controller;

use header::Header;
pub use mmu::{AccessType, MemoryValue};
use scheduler::Scheduler;
use interrupt_controller::InterruptController;

pub struct HW {
    bios7: Vec<u8>,
    bios9: Vec<u8>,
    rom_header: Header,
    rom: Vec<u8>,
    main_mem: Vec<u8>,
    iwram: Vec<u8>,

    interrupts7: InterruptController,
    interrupts9: InterruptController,
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

            interrupts7: InterruptController::new(),
            interrupts9: InterruptController::new(),
            scheduler: Scheduler::new(),
        }
    }

    pub fn clock(&mut self) {
        self.handle_events();
    }

    pub fn arm7_interrupts_requested(&self) -> bool {
        self.interrupts7.interrupts_requested()
    }

    pub fn arm9_interrupts_requested(&self) -> bool {
        self.interrupts9.interrupts_requested()
    }
}
