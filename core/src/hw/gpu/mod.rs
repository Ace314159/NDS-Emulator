mod registers;
mod engine2d;
mod vram;

use crate::hw::interrupt_controller::InterruptRequest;
use registers::{DISPSTAT, DISPSTATFlags};
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
}

impl GPU {
    pub const WIDTH: usize = 256;
    pub const HEIGHT: usize = 192;

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
        }
    }

    pub fn emulate_dot(&mut self) -> InterruptRequest {
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
                // TODO: HBlank DMA
                //if self.vcount < GPU::HEIGHT { self.hblank_called = true } // HDMA only occurs on visible scanlines
            }
        }
        if self.vcount < GPU::HEIGHT as u16 { // Visible
            self.dispstat.remove(DISPSTATFlags::VBLANK);
            if self.dot == 257 {
                self.engine_a.render_line(&self.vram, self.vcount);
                self.engine_b.render_line(&self.vram, self.vcount);
            }
        } else { // VBlank
            if self.vcount == GPU::HEIGHT as u16 && self.dot == 0 {
                // TODO: VBlank DMA
                //self.vblank_called = true;
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
        [&self.engine_a.pixels, &self.engine_b.pixels]
    }
}
