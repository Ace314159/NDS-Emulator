mod cartridge;
mod dma;
mod gpu;
mod interrupt_controller;
mod ipc;
mod keypad;
mod math;
pub mod mem;
mod scheduler;
mod rtc;
mod spi;
mod spu;
mod timers;

use std::convert::TryInto;
use std::fs::File;

use crate::unlikely;
use cartridge::Cartridge;
pub use gpu::{EngineA, EngineB, GPU};
use interrupt_controller::{InterruptController, InterruptRequest};
use ipc::IPC;
pub use keypad::Key;
use keypad::Keypad;
use math::{Div, Sqrt};
pub use mem::{AccessType, MemoryValue};
use mem::{CP15, EXMEM, HALTCNT, POWCNT2, WRAMCNT};
use scheduler::Scheduler;
use rtc::RTC;
use spi::SPI;
use spu::SPU;
use timers::Timers;

pub struct HW {
    // Memory
    pub cp15: CP15,
    bios7: Vec<u8>,
    bios9: Vec<u8>,
    cartridge: Cartridge,
    itcm: Vec<u8>,
    dtcm: Vec<u8>,
    main_mem: Vec<u8>,
    iwram: Vec<u8>,
    shared_wram: Vec<u8>,
    arm7_page_table: Vec<*mut u8>,
    arm9_page_table: Vec<*mut u8>,
    // Devices
    pub gpu: GPU,
    spu: SPU,
    keypad: Keypad,
    interrupts: [InterruptController; 2],
    dmas: [dma::Controller; 2],
    dma_fill: [u32; 4],
    timers: [Timers; 2],
    ipc: IPC,
    rtc: RTC,
    spi: SPI,
    // Registers
    wramcnt: WRAMCNT,
    powcnt2: POWCNT2,
    pub haltcnt: HALTCNT,
    postflg7: u8,
    postflg9: u8,
    exmem: EXMEM,
    // Math
    div: Div,
    sqrt: Sqrt,
    // Misc
    scheduler: Scheduler,
}

impl HW {
    const ITCM_SIZE: usize = 0x8000;
    const DTCM_SIZE: usize = 0x4000;
    const MAIN_MEM_SIZE: usize = 0x40_0000;
    const IWRAM_SIZE: usize = 0x1_0000;
    const SHARED_WRAM_SIZE: usize = 0x8000;

    pub fn new(
        bios7: Vec<u8>,
        bios9: Vec<u8>,
        firmware_file: File,
        rom: Vec<u8>,
        save_file: File,
        direct_boot: bool,
    ) -> Self {
        let mut scheduler = Scheduler::new();
        let cartridge = Cartridge::new(rom, save_file, &bios7);
        let mut hw = HW {
            // Memory
            cp15: CP15::new(),
            bios7,
            bios9,
            cartridge,
            itcm: vec![0; HW::ITCM_SIZE],
            dtcm: vec![0; HW::DTCM_SIZE],
            main_mem: vec![0; HW::MAIN_MEM_SIZE],
            iwram: vec![0; HW::IWRAM_SIZE],
            shared_wram: vec![0; HW::SHARED_WRAM_SIZE],
            arm7_page_table: vec![std::ptr::null_mut(); HW::ARM7_PAGE_TABLE_SIZE],
            arm9_page_table: vec![std::ptr::null_mut(); HW::ARM9_PAGE_TABLE_SIZE],
            // Devices
            gpu: GPU::new(&mut scheduler),
            spu: SPU::new(&mut scheduler),
            keypad: Keypad::new(),
            interrupts: [InterruptController::new(), InterruptController::new()],
            dmas: [dma::Controller::new(false), dma::Controller::new(true)],
            dma_fill: [0; 4],
            timers: [Timers::new(false), Timers::new(true)],
            ipc: IPC::new(),
            rtc: RTC::new(),
            spi: SPI::new(firmware_file),
            // Registesr
            wramcnt: WRAMCNT::new(3),
            powcnt2: POWCNT2::new(),
            haltcnt: HALTCNT::new(),
            postflg7: if direct_boot { 0x1 } else { 0x0 },
            postflg9: if direct_boot { 0x1 } else { 0x0 },
            exmem: EXMEM::new(),
            // Math
            div: Div::new(),
            sqrt: Sqrt::new(),
            // Misc
            scheduler,
        };
        hw.init_arm7_page_tables();
        hw.init_arm9_page_tables();
        if direct_boot {
            hw.init_mem()
        } else {
            hw.cartridge.encrypt_secure_area();
            hw
        }
    }

    pub fn clock_until(&mut self, target: usize) {
        self.handle_events(target);
    }

    pub fn arm7_interrupts_requested(&mut self) -> bool {
        if unlikely(self.keypad.interrupt_requested()) {
            self.interrupts[0].request |= InterruptRequest::KEYPAD
        }
        self.interrupts[0].interrupts_requested(self.haltcnt.halted())
    }

    pub fn arm9_interrupts_requested(&mut self) -> bool {
        if unlikely(self.keypad.interrupt_requested()) {
            self.interrupts[1].request |= InterruptRequest::KEYPAD
        }
        self.interrupts[1].interrupts_requested(false)
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

    pub fn press_screen(&mut self, x: usize, y: usize) {
        self.keypad.press_screen();
        self.spi.press_screen(x, y)
    }

    pub fn release_screen(&mut self) {
        self.keypad.release_screen();
        self.spi.release_screen();
    }

    pub fn render_palettes(
        &self,
        extended: bool,
        slot: usize,
        palette: usize,
        engine: Engine,
        graphics_type: GraphicsType,
    ) -> (Vec<u16>, usize, usize) {
        if extended {
            match (engine, graphics_type) {
                (Engine::A, GraphicsType::BG) => GPU::render_palettes(
                    |i| {
                        self.gpu
                            .vram
                            .get_bg_ext_pal::<EngineA>(slot, palette * 256 + i)
                    },
                    16,
                ),
                (Engine::A, GraphicsType::OBJ) => GPU::render_palettes(
                    |i| self.gpu.vram.get_obj_ext_pal::<EngineA>(palette * 256 + i),
                    16,
                ),
                (Engine::B, GraphicsType::BG) => GPU::render_palettes(
                    |i| {
                        self.gpu
                            .vram
                            .get_bg_ext_pal::<EngineB>(slot, palette * 256 + i)
                    },
                    16,
                ),
                (Engine::B, GraphicsType::OBJ) => GPU::render_palettes(
                    |i| self.gpu.vram.get_obj_ext_pal::<EngineB>(palette * 256 + i),
                    16,
                ),
            }
        } else {
            match (engine, graphics_type) {
                (Engine::A, GraphicsType::BG) => {
                    GPU::render_palettes(|i| self.gpu.engine_a.bg_palettes()[i], 16)
                }
                (Engine::A, GraphicsType::OBJ) => {
                    GPU::render_palettes(|i| self.gpu.engine_a.obj_palettes()[i], 16)
                }
                (Engine::B, GraphicsType::BG) => {
                    GPU::render_palettes(|i| self.gpu.engine_b.bg_palettes()[i], 16)
                }
                (Engine::B, GraphicsType::OBJ) => {
                    GPU::render_palettes(|i| self.gpu.engine_b.obj_palettes()[i], 16)
                }
            }
        }
    }

    pub fn render_map(&self, engine: Engine, bg_i: usize) -> (Vec<u16>, usize, usize) {
        match engine {
            Engine::A => self.gpu.engine_a.render_map(&self.gpu.vram, bg_i),
            Engine::B => self.gpu.engine_b.render_map(&self.gpu.vram, bg_i),
        }
    }

    pub fn render_tiles(
        &self,
        engine: Engine,
        graphics_type: GraphicsType,
        extended: bool,
        bitmap: bool,
        bpp8: bool,
        slot: usize,
        palette: usize,
        offset: usize,
    ) -> (Vec<u16>, usize, usize) {
        let is_bg = graphics_type == GraphicsType::BG;
        match engine {
            Engine::A => self.gpu.engine_a.render_tiles(
                &self.gpu.vram,
                is_bg,
                extended,
                bitmap,
                bpp8,
                slot,
                palette,
                offset,
            ),
            Engine::B => self.gpu.engine_b.render_tiles(
                &self.gpu.vram,
                is_bg,
                extended,
                bitmap,
                bpp8,
                slot,
                palette,
                offset,
            ),
        }
    }

    pub fn render_bank(&self, ignore_alpha: bool, bank: usize) -> (Vec<u16>, usize, usize) {
        self.gpu.vram.render_bank(ignore_alpha, bank)
    }

    pub fn init_mem(mut self) -> Self {
        let addr = 0x027F_FE00 & (HW::MAIN_MEM_SIZE - 1);
        self.main_mem[addr..addr + 0x170].copy_from_slice(&self.cartridge.rom()[..0x170]);

        for addr in [0x027FF800, 0x027FFC00].iter() {
            self.arm9_write(addr + 0x0, self.cartridge.chip_id());
            self.arm9_write(addr + 0x4, self.cartridge.chip_id());
            self.arm9_write(
                addr + 0x8,
                u16::from_le_bytes(self.cartridge.rom()[0x15E..=0x15F].try_into().unwrap()),
            );
            self.arm9_write(
                addr + 0xA,
                u16::from_le_bytes(self.cartridge.rom()[0x6C..=0x6D].try_into().unwrap()),
            );
        }

        self.arm9_write(0x027FF850, 0x5835u16);
        self.arm9_write(0x027FFC10, 0x5835u16);
        self.arm9_write(0x027FFC30, 0xFFFFu16);
        self.arm9_write(0x027FFC40, 0x0001u16);
        self
    }

    fn map_page_table(
        page_table: &mut [*mut u8],
        page_shift: usize,
        page_size: usize,
        addr_start: usize,
        addr_end: usize,
        mem: &mut [u8],
    ) {
        let mem_mask = mem.len() - 1;
        let mut page_table_i = (addr_start as usize) >> page_shift;
        for addr in (addr_start..addr_end).step_by(page_size) {
            let mem_addr = addr & mem_mask;
            page_table[page_table_i] = mem[mem_addr..mem_addr + page_size].as_mut_ptr();
            page_table_i += 1;
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Engine {
    A = 0,
    B = 1,
}

impl Engine {
    pub fn label(&self) -> &str {
        match self {
            Engine::A => "A",
            Engine::B => "B",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum GraphicsType {
    BG,
    OBJ,
}

impl GraphicsType {
    pub fn label(&self) -> &str {
        match self {
            GraphicsType::BG => "BG",
            GraphicsType::OBJ => "OBJ",
        }
    }
}
