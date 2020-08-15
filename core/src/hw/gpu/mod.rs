mod registers;
mod engine2d;
mod engine3d;
mod vram;
pub mod debug;

use crate::hw::{
    Event, Scheduler,
};
use registers::POWCNT1;

pub use engine2d::Engine2D;
pub use engine3d::Engine3D;
pub use vram::VRAM;
pub use registers::{DISPSTAT, DISPSTATFlags};

pub struct GPU {
    // Registers and Values Shared between Engines
    pub dispstat7: DISPSTAT,
    pub dispstat9: DISPSTAT,
    pub vcount: u16,
    rendered_frame: bool,

    pub engine_a: Engine2D<EngineA>,
    pub engine_b: Engine2D<EngineB>,
    pub engine3d: Engine3D,
    pub vram: VRAM,

    pub powcnt1: POWCNT1,
}

impl GPU {
    pub const WIDTH: usize = 256;
    pub const HEIGHT: usize = 192;

    pub const PALETTE_SIZE: usize = 0x200;
    pub const OAM_SIZE: usize = 0x400;
    pub const OAM_MASK: usize = GPU::OAM_SIZE - 1;

    const CYCLES_PER_DOT: usize = 6;
    const HBLANK_DOT: usize = 256 + 8;
    const DOTS_PER_LINE: usize = 355;
    const NUM_LINES: usize = 263;

    pub fn new(scheduler: &mut Scheduler) -> GPU {
        scheduler.schedule(Event::HBlank, GPU::HBLANK_DOT * GPU::CYCLES_PER_DOT);
        GPU {
            // Registers and Values Shared between Engines
            dispstat7: DISPSTAT::new(),
            dispstat9: DISPSTAT::new(),
            vcount: 0,
            rendered_frame: false,

            engine_a: Engine2D::new(),
            engine_b: Engine2D::new(),
            engine3d: Engine3D::new(),
            vram: VRAM::new(),

            powcnt1: POWCNT1::ENABLE_LCDS,
        }
    }

    // Dot: 0 - TODO: Check for drift
    pub fn start_next_line(&mut self, scheduler: &mut Scheduler) -> (u16, bool) {
        scheduler.schedule(Event::HBlank, GPU::HBLANK_DOT * GPU::CYCLES_PER_DOT);
        self.dispstat7.remove(DISPSTATFlags::HBLANK);
        self.dispstat9.remove(DISPSTATFlags::HBLANK);

        if self.vcount == 262 {
            self.engine_a.latch_affine();
            self.engine_b.latch_affine();
        }
        self.vcount += 1;
        if self.vcount == GPU::NUM_LINES as u16 {
            self.vcount = 0;
        }
        
        let start_vblank = if self.vcount == 0 {
            self.dispstat7.remove(DISPSTATFlags::VBLANK);
            self.dispstat9.remove(DISPSTATFlags::VBLANK);
            false
        } else if self.vcount == GPU::HEIGHT as u16 {
            self.dispstat7.insert(DISPSTATFlags::VBLANK);
            self.dispstat9.insert(DISPSTATFlags::VBLANK);
            self.rendered_frame = true;
            true
        } else { false };
        (self.vcount, start_vblank)
    }

    // Dot: HBLANK_DOT - TODO: Check for drift
    pub fn start_hblank(&mut self, scheduler: &mut Scheduler) -> bool {
        scheduler.schedule(Event::StartNextLine, (GPU::DOTS_PER_LINE - GPU::HBLANK_DOT) * GPU::CYCLES_PER_DOT);
        self.dispstat7.insert(DISPSTATFlags::HBLANK);
        self.dispstat9.insert(DISPSTATFlags::HBLANK);

        if self.vcount < GPU::HEIGHT as u16 {
            // TOOD: Use POWCNT to selectively render engines
            self.engine_a.render_line(&self.engine3d, &self.vram, self.vcount);
            self.engine_b.render_line(&self.engine3d, &self.vram, self.vcount);
            true
        } else { false }
    }

    pub fn rendered_frame(&mut self) -> bool {
        let rendered_frame = self.rendered_frame;
        self.rendered_frame = false;
        rendered_frame
    }

    pub fn get_screens(&self) -> [&Vec<u16>; 2] {
        if self.powcnt1.contains(POWCNT1::TOP_A) {
            [&self.engine_a.pixels(), &self.engine_b.pixels()]
        } else {
            [&self.engine_b.pixels(), &self.engine_a.pixels()]
        }
    }
}

pub trait EngineType {
    fn is_a() -> bool;
}

pub struct EngineA {}
pub struct EngineB {}

impl EngineType for EngineA { fn is_a() -> bool { true }}
impl EngineType for EngineB { fn is_a() -> bool { false }}
