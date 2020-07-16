mod header;
pub mod mmu;
mod scheduler;
mod gpu;
mod keypad;
mod interrupt_controller;
mod dma;
mod timers;
mod ipc;

use header::Header;
pub use mmu::{AccessType, MemoryValue};
use mmu::{WRAMCNT, CP15};
use scheduler::{Scheduler, Event, EventType};
pub use gpu::GPU;
use keypad::Keypad;
pub use keypad::Key;
use interrupt_controller::{InterruptController, InterruptRequest};
use dma::DMAController;
use timers::Timers;
use ipc::IPC;

pub struct HW {
    // Memory
    pub cp15: CP15,
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
    dma7: DMAController,
    dma9: DMAController,
    dma_fill: [u32; 4],
    timers7: Timers,
    timers9: Timers,
    ipc: IPC,
    // Registers
    wramcnt: WRAMCNT,
    // Misc
    arm7_cycles_ahead: usize,
    scheduler: Scheduler,
}

impl HW {
    const ITCM_SIZE: usize = 0x8000;
    const DTCM_SIZE: usize = 0x4000;
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
            cp15: CP15::new(),
            bios7,
            bios9,
            rom_header,
            rom,
            itcm: vec![0; HW::ITCM_SIZE],
            dtcm: vec![0; HW::DTCM_SIZE],
            main_mem,
            iwram: vec![0; HW::IWRAM_SIZE],
            shared_wram: vec![0; HW::SHARED_WRAM_SIZE],
            // Devices
            gpu: GPU::new(),
            keypad: Keypad::new(),
            interrupts7: InterruptController::new(),
            interrupts9: InterruptController::new(),
            dma7: DMAController::new(false),
            dma9: DMAController::new(true),
            dma_fill: [0; 4],
            timers7: Timers::new(false),
            timers9: Timers::new(true),
            ipc: IPC::new(),
            // Registesr
            wramcnt: WRAMCNT::new(3),
            // Misc
            arm7_cycles_ahead: 0,
            scheduler: Scheduler::new(),
        }
    }

    pub fn clock(&mut self, arm7_cycles: usize) {
        self.arm7_cycles_ahead += arm7_cycles;
        while self.arm7_cycles_ahead >= 6 {
            self.arm7_cycles_ahead -= 6;
            let interrupts = self.gpu.emulate_dot(&mut self.scheduler);
            self.interrupts7.request |= interrupts;
            self.interrupts9.request |= interrupts;
        }
        self.handle_events(arm7_cycles);
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
