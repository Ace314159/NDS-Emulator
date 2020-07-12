mod header;
pub mod mmu;
mod scheduler;
mod gpu;
mod keypad;
mod interrupt_controller;

use header::Header;
pub use mmu::{AccessType, MemoryValue};
use mmu::WRAMCNT;
use scheduler::Scheduler;
pub use gpu::GPU;
use keypad::Keypad;
pub use keypad::Key;
use interrupt_controller::{InterruptController, InterruptRequest};

pub struct HW {
    // Memory
    bios7: Vec<u8>,
    bios9: Vec<u8>,
    rom_header: Header,
    rom: Vec<u8>,
    itcm: Vec<u8>,
    dtcm: Vec<u8>,
    main_mem: Vec<u8>,
    iwram: Vec<u8>,
    shared_wram: Vec<u8>,
    // Devices
    pub gpu: GPU,
    keypad: Keypad,
    interrupts7: InterruptController,
    interrupts9: InterruptController,
    // Registers
    wramcnt: WRAMCNT,
    // Misc
    scheduler: Scheduler,
}

impl HW {
    const ITCM_SIZE: usize = 0x8000;
    const MAIN_MEM_SIZE: usize = 0x20_0000;
    const IWRAM_SIZE: usize = 0x1_0000;
    const SHARED_WRAM_SIZE: usize = 0x8000;

    pub fn new(bios7: Vec<u8>, bios9: Vec<u8>, rom: Vec<u8>) -> Self {
        let mut main_mem = vec![0; HW::MAIN_MEM_SIZE];
        let rom_header = Header::new(&rom);
        let addr = 0x027F_FE00 & (HW::MAIN_MEM_SIZE - 1); 
        main_mem[addr..addr + 0x160].copy_from_slice(&rom[..0x160]);
        HW {
            // Memory
            bios7,
            bios9,
            rom_header,
            rom,
            itcm: vec![0; HW::ITCM_SIZE],
            dtcm: vec![0; 0x4000],
            main_mem,
            iwram: vec![0; HW::IWRAM_SIZE],
            shared_wram: vec![0; HW::SHARED_WRAM_SIZE],
            // Devices
            gpu: GPU::new(),
            keypad: Keypad::new(),
            interrupts7: InterruptController::new(),
            interrupts9: InterruptController::new(),
            // Registesr
            wramcnt: WRAMCNT::new(3),
            // Misc
            scheduler: Scheduler::new(),
        }
    }

    pub fn clock(&mut self) {
        self.handle_events();
        self.gpu.emulate_dot();
    }

    pub fn arm7_interrupts_requested(&mut self) -> bool {
        if self.keypad.interrupt_requested() { self.interrupts7.request |= InterruptRequest::KEYPAD }
        self.interrupts7.interrupts_requested()
    }

    pub fn arm9_interrupts_requested(&mut self) -> bool {
        if self.keypad.interrupt_requested() { self.interrupts9.request |= InterruptRequest::KEYPAD }
        self.interrupts9.interrupts_requested()
    }

    pub fn rendered_frame(&mut self) -> bool {
        self.gpu.rendered_frame()
    }

    pub fn press_key(&mut self, key: Key) {
        self.keypad.press_key(key);
    }

    pub fn release_key(&mut self, key: Key) {
        self.keypad.release_key(key);
    }
}
