pub mod mmu;
mod scheduler;
mod gpu;
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

pub use mmu::{AccessType, MemoryValue};
use mmu::{CP15, EXMEM, HALTCNT, POWCNT2, WRAMCNT};
use scheduler::{Scheduler, Event};
pub use gpu::{GPU, EngineA, EngineB};
use keypad::Keypad;
pub use keypad::Key;
use interrupt_controller::{InterruptController, InterruptRequest};
use dma::{DMAController, DMAOccasion};
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
    keypad: Keypad,
    interrupts7: InterruptController,
    interrupts9: InterruptController,
    dma7: DMAController,
    dma9: DMAController,
    dma_fill: [u32; 4],
    timers7: Timers,
    timers9: Timers,
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

    pub fn new(bios7: Vec<u8>, bios9: Vec<u8>, firmware: Vec<u8>, rom: Vec<u8>, save_file: PathBuf) -> Self {
        let mut scheduler = Scheduler::new();
        HW {
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
            keypad: Keypad::new(),
            interrupts7: InterruptController::new(),
            interrupts9: InterruptController::new(),
            dma7: DMAController::new(false),
            dma9: DMAController::new(true),
            dma_fill: [0; 4],
            timers7: Timers::new(false),
            timers9: Timers::new(true),
            ipc: IPC::new(),
            spi: SPI::new(firmware),
            // Registesr
            wramcnt: WRAMCNT::new(3),
            powcnt2: POWCNT2::new(),
            haltcnt: HALTCNT::new(),
            postflg7: 0x1, // TODO: Set to 1 after boot
            postflg9: 0x1, // TODO: Set to 1 after boot
            exmem: EXMEM::new(),
            // Math
            div: Div::new(),
            sqrt: Sqrt::new(),
            // Misc
            scheduler,
        }.init_mem()
    }

    pub fn clock(&mut self, arm7_cycles: usize) {
        self.handle_events(arm7_cycles);
        if self.gpu.powcnt1.contains(gpu::POWCNT1::ENABLE_3D_GEOMETRY) &&
            self.gpu.engine3d.clock(&mut self.interrupts9.request) {
            self.run_dmas(DMAOccasion::GeometryCommandFIFO)
        }
    }

    fn run_dmas(&mut self, occasion: DMAOccasion) {
        let mut events = Vec::new();
        for num in self.dma9.by_type[occasion as usize].iter() {
            events.push(Event::DMA(true, *num));
        }
        for num in self.dma7.by_type[occasion as usize].iter() {
            events.push(Event::DMA(false, *num));
        }
        for event in events.iter() { self.handle_event(*event) }
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
