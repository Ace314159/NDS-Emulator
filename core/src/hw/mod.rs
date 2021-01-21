pub mod mem;
mod scheduler;
mod gpu;
mod spu;
mod keypad;
mod interrupt_controller;
mod dma;
mod timers;
mod ipc;
mod math;
mod spi;
mod cartridge;

use std::convert::TryInto;
use std::path::PathBuf;

pub use mem::{AccessType, MemoryValue};
use mem::{CP15, EXMEM, HALTCNT, POWCNT2, WRAMCNT};
use scheduler::Scheduler;
pub use gpu::{GPU, EngineA, EngineB};
use spu::SPU;
use keypad::Keypad;
pub use keypad::Key;
use interrupt_controller::{InterruptController, InterruptRequest};
use dma::DMAController;
use timers::Timers;
use ipc::IPC;
use math::{Div, Sqrt};
use spi::SPI;
use cartridge::Cartridge;

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
    // Devices
    pub gpu: GPU,
    spu: SPU,
    keypad: Keypad,
    interrupts: [InterruptController; 2],
    in_dma: bool,
    dmas: [DMAController; 2],
    dma_fill: [u32; 4],
    timers: [Timers; 2],
    ipc: IPC,
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

    pub fn new(bios7: Vec<u8>, bios9: Vec<u8>, firmware: Vec<u8>, rom: Vec<u8>, save_file: PathBuf, direct_boot: bool) -> Self {
        let mut scheduler = Scheduler::new();
        let hw = HW {
            // Memory
            cp15: CP15::new(),
            bios7,
            bios9,
            cartridge: Cartridge::new(rom, save_file),
            itcm: vec![0; HW::ITCM_SIZE],
            dtcm: vec![0; HW::DTCM_SIZE],
            main_mem: vec![0; HW::MAIN_MEM_SIZE],
            iwram: vec![0; HW::IWRAM_SIZE],
            shared_wram: vec![0; HW::SHARED_WRAM_SIZE],
            // Devices
            gpu: GPU::new(&mut scheduler),
            spu: SPU::new(&mut scheduler),
            keypad: Keypad::new(),
            interrupts: [InterruptController::new(), InterruptController::new()],
            in_dma: false,
            dmas: [DMAController::new(false), DMAController::new(true)],
            dma_fill: [0; 4],
            timers: [Timers::new(false), Timers::new(true)],
            ipc: IPC::new(),
            spi: SPI::new(firmware),
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
        if direct_boot { hw.init_mem() } else { hw }
    }

    pub fn clock(&mut self, arm7_cycles: usize) {
        self.handle_events(arm7_cycles);
        self.gpu.engine3d.check_interrupts(&mut self.interrupts[1].request);
    }

    pub fn arm7_interrupts_requested(&mut self) -> bool {
        if self.keypad.interrupt_requested() { self.interrupts[0].request |= InterruptRequest::KEYPAD }
        self.interrupts[0].interrupts_requested()
    }

    pub fn arm9_interrupts_requested(&mut self) -> bool {
        if self.keypad.interrupt_requested() { self.interrupts[1].request |= InterruptRequest::KEYPAD }
        self.interrupts[1].interrupts_requested()
    }

    pub fn rendered_frame(&mut self) -> bool {
        self.gpu.rendered_frame()
    }

    pub fn save_backup(&mut self) {
        self.cartridge.save_backup();
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

    pub fn render_palettes(&self, extended: bool, slot: usize, palette: usize,
        engine: Engine, graphics_type: GraphicsType) -> (Vec<u16>, usize, usize) {
        if extended {
            match (engine, graphics_type) {
                (Engine::A, GraphicsType::BG) => GPU::render_palettes(|i|
                    self.gpu.vram.get_bg_ext_pal::<EngineA>(slot, palette * 256 + i), 16),
                (Engine::A, GraphicsType::OBJ) => GPU::render_palettes(|i|
                    self.gpu.vram.get_obj_ext_pal::<EngineA>(palette * 256 + i), 16),
                (Engine::B, GraphicsType::BG) => GPU::render_palettes(|i|
                    self.gpu.vram.get_bg_ext_pal::<EngineB>(slot, palette * 256 + i), 16),
                (Engine::B, GraphicsType::OBJ) => GPU::render_palettes(|i|
                    self.gpu.vram.get_obj_ext_pal::<EngineB>(palette * 256 + i), 16),
            }
        } else {
            match (engine, graphics_type) {
                (Engine::A, GraphicsType::BG) =>
                    GPU::render_palettes(|i| self.gpu.engine_a.bg_palettes()[i], 16),
                (Engine::A, GraphicsType::OBJ) =>
                    GPU::render_palettes(|i| self.gpu.engine_a.obj_palettes()[i], 16),
                (Engine::B, GraphicsType::BG) =>
                    GPU::render_palettes(|i| self.gpu.engine_b.bg_palettes()[i], 16),
                (Engine::B, GraphicsType::OBJ) =>
                    GPU::render_palettes(|i| self.gpu.engine_b.obj_palettes()[i], 16),
            }
        }
    }

    pub fn render_map(&self, engine: Engine, bg_i: usize) -> (Vec<u16>, usize, usize) {
        match engine {
            Engine::A => self.gpu.engine_a.render_map(&self.gpu.vram, bg_i),
            Engine::B => self.gpu.engine_b.render_map(&self.gpu.vram, bg_i),
        }
    }

    pub fn render_tiles(&self, engine: Engine, graphics_type: GraphicsType, extended: bool, bitmap: bool, bpp8: bool,
        slot: usize, palette: usize, offset: usize) -> (Vec<u16>, usize, usize) {
        let is_bg = graphics_type == GraphicsType::BG;
        match engine {
            Engine::A => self.gpu.engine_a.render_tiles(&self.gpu.vram, is_bg, extended, bitmap, bpp8, slot, palette, offset),
            Engine::B => self.gpu.engine_b.render_tiles(&self.gpu.vram, is_bg, extended, bitmap, bpp8, slot, palette, offset),
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
            self.arm9_write(addr + 0x8, u16::from_le_bytes(self.cartridge.rom()[0x15E..=0x15F].try_into().unwrap()));
            self.arm9_write(addr + 0xA, u16::from_le_bytes(self.cartridge.rom()[0x6C..=0x6D].try_into().unwrap()));
        }

        self.arm9_write(0x027FF850, 0x5835u16);
        self.arm9_write(0x027FFC10, 0x5835u16);
        self.arm9_write(0x027FFC30, 0xFFFFu16);
        self.arm9_write(0x027FFC40, 0x0001u16);
        self
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
