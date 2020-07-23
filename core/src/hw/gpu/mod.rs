mod registers;
mod engine2d;
mod vram;
pub mod debug;

use crate::hw::{
    interrupt_controller::InterruptRequest,
    Event, Scheduler,
};
use registers::{DISPSTAT, DISPSTATFlags, POWCNT1};
pub use engine2d::Engine2D;
use vram::VRAM;

pub struct GPU {
    // Registers and Values Shared between Engines
    pub dispstat: DISPSTAT,
    pub vcount: u16,
    dot: u16,
    rendered_frame: bool,

    pub engine_a: Engine2D,
    pub engine_b: Engine2D,
    pub vram: VRAM,

    pub powcnt1: POWCNT1,
}

impl GPU {
    pub const WIDTH: usize = 256;
    pub const HEIGHT: usize = 192;

    pub const PALETTE_SIZE: usize = 0x400;
    pub const PALETTE_MASK: usize = GPU::PALETTE_SIZE - 1;
    pub const OAM_SIZE: usize = 0x400;
    pub const OAM_MASK: usize = GPU::OAM_SIZE - 1;

    pub fn new() -> GPU {
        GPU {
            // Registers and Values Shared between Engines
            dispstat: DISPSTAT::new(),
            vcount: 0,
            dot: 0,
            rendered_frame: false,

            engine_a: Engine2D::new(),
            engine_b: Engine2D::new(),
            vram: VRAM::new(),

            powcnt1: POWCNT1::ENABLE_LCDS,
        }
    }

    pub fn emulate_dot(&mut self, scheduler: &mut Scheduler) -> InterruptRequest {
        // TODO: Optimize
        let mut interrupts = InterruptRequest::empty();
        if self.dot < GPU::WIDTH as u16 { // Visible
            self.dispstat.remove(DISPSTATFlags::HBLANK);
        } else { // HBlank
            if self.dot == GPU::WIDTH as u16 {
                if self.dispstat.contains(DISPSTATFlags::HBLANK_IRQ_ENABLE) {
                    interrupts.insert(InterruptRequest::HBLANK);
                }
            }
            if self.dot == 267 { // TODO: Take into account half and differentiate between ARM7 and ARM9
                self.dispstat.insert(DISPSTATFlags::HBLANK);
                if self.vcount < GPU::HEIGHT as u16 {
                    scheduler.run_now(Event::HBlank);
                }
            }
        }
        if self.vcount < GPU::HEIGHT as u16 { // Visible
            self.dispstat.remove(DISPSTATFlags::VBLANK);
            if self.dot == 257 {
                // TOOD: Use POWCNT to selectively render engines
                self.engine_a.render_line(&self.vram, &VRAM::get_engine_a_bg,
                    &VRAM::get_engine_a_obj, self.vcount);
                self.engine_b.render_line(&self.vram, &VRAM::get_engine_b_bg,
                    &VRAM::get_engine_b_obj, self.vcount);
            }
        } else { // VBlank
            if self.vcount == GPU::HEIGHT as u16 && self.dot == 0 {
                scheduler.run_now(Event::VBlank);
                if self.dispstat.contains(DISPSTATFlags::VBLANK_IRQ_ENABLE) {
                    interrupts.insert(InterruptRequest::VBLANK)
                }
            }
            self.dispstat.insert(DISPSTATFlags::VBLANK);
        }

        if self.vcount == GPU::HEIGHT as u16 && self.dot == 0 {
            self.rendered_frame = true;
        }

        self.dot += 1;
        if self.dot == 355 {
            self.dot = 0;
            if self.vcount == 262 {
                self.engine_a.latch_affine();
                self.engine_a.latch_affine();
            }
            self.vcount = (self.vcount + 1) % 263;
            if self.vcount == self.dispstat.vcount_setting {
                self.dispstat.insert(DISPSTATFlags::VCOUNTER);
                if self.dispstat.contains(DISPSTATFlags::VCOUNTER_IRQ_ENALBE) {
                    interrupts.insert(InterruptRequest::VCOUNTER_MATCH);
                }
            } else {
                self.dispstat.remove(DISPSTATFlags::VCOUNTER);
            }
        }
        interrupts
    }

    pub fn rendered_frame(&mut self) -> bool {
        let rendered_frame = self.rendered_frame;
        self.rendered_frame = false;
        rendered_frame
    }

    pub fn get_screens(&self) -> [&Vec<u16>; 2] {
        if self.powcnt1.contains(POWCNT1::SWAP_DISPLAY) {
            [&self.engine_b.pixels, &self.engine_a.pixels]
        } else {
            [&self.engine_a.pixels, &self.engine_b.pixels]
        }
    }
}
