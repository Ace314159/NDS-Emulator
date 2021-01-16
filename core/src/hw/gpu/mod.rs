mod registers;
mod engine2d;
mod engine3d;
mod vram;
pub mod debug;

use crate::hw::{
    HW,
    scheduler::{Event, Scheduler},
    interrupt_controller::{InterruptController, InterruptRequest},
    dma::DMAOccasion,
};

pub use engine2d::Engine2D;
pub use engine3d::Engine3D;
pub use vram::VRAM;
pub use registers::{DISPSTAT, DISPSTATFlags, DISPCAPCNT, POWCNT1};

use registers::CaptureSource;
use engine2d::DisplayMode;

pub struct GPU {
    // Registers and Values Shared between Engines
    pub dispstats: [DISPSTAT; 2],
    pub vcount: u16,
    rendered_frame: bool,

    pub engine_a: Engine2D<EngineA>,
    pub engine_b: Engine2D<EngineB>,
    pub engine3d: Engine3D,
    pub vram: VRAM,

    pub dispcapcnt: DISPCAPCNT,
    capturing: bool,
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
            dispstats: [DISPSTAT::new(), DISPSTAT::new()],
            vcount: 0,
            rendered_frame: false,

            engine_a: Engine2D::new(),
            engine_b: Engine2D::new(),
            engine3d: Engine3D::new(),
            vram: VRAM::new(),

            dispcapcnt: DISPCAPCNT::new(),
            capturing: false,
            powcnt1: POWCNT1::ENABLE_LCDS,
        }
    }

    // Dot: 0 - TODO: Check for drift
    pub fn start_next_line(&mut self) {
        for dispstat in self.dispstats.iter_mut() { dispstat.remove(DISPSTATFlags::HBLANK) }

        if self.vcount == 262 {
            self.engine_a.latch_affine();
            self.engine_b.latch_affine();
        }
        self.vcount += 1;
        if self.vcount == GPU::NUM_LINES as u16 {
            self.vcount = 0;
        }
    }

    // Dot: HBLANK_DOT - TODO: Check for drift
    pub fn render_line(&mut self) {
        // TODO: Use POWCNT to selectively render engines
        if self.powcnt1.contains(POWCNT1::ENABLE_ENGINE_A) {
            self.engine_a.render_line(&self.engine3d, &self.vram, self.vcount);
            if self.capturing && (self.vcount as usize) < self.dispcapcnt.capture_size.height() {
                self.capture();
            }
        }
        if self.powcnt1.contains(POWCNT1::ENABLE_ENGINE_B) {
            self.engine_b.render_line(&self.engine3d, &self.vram, self.vcount)
        }
    }

    pub fn capture(&mut self) {
        let start_addr = self.vcount as usize * GPU::WIDTH;
        let width = self.dispcapcnt.capture_size.width();
        let src_a = &if self.dispcapcnt.src_a_is_3d_only ||
        self.engine_a.dispcnt.display_mode != DisplayMode::Mode0 {
            self.engine3d.pixels()
        } else { self.engine_a.pixels() }[start_addr..start_addr + width];
        let mut src_b = [0; 2 * GPU::WIDTH];
        if self.dispcapcnt.src_b_fifo {
            todo!()
        } else {
            let offset = 2 * start_addr + if self.engine_a.dispcnt.display_mode == DisplayMode::Mode2 {
                0
            } else { self.dispcapcnt.vram_read_offset.offset() };
            let block = self.engine_a.dispcnt.vram_block as usize;
            // TODO: Figure out how to avoid this copy and keep borrow checker happy
            src_b[..2 * width].copy_from_slice(&self.vram.banks[block][offset..offset + 2 * width]);
        }

        let offset = 2 * start_addr + self.dispcapcnt.vram_write_offset.offset();
        let bank = &mut self.vram.banks[self.dispcapcnt.vram_write_block];
        // TODO: Replace write_mem and read_mem with slice conversions
        match self.dispcapcnt.capture_src {
            CaptureSource::A => for (i, pixel) in src_a.iter().enumerate() {
                HW::write_mem(bank, offset as u32 + 2 * i as u32, *pixel);
            },
            CaptureSource::B =>
                bank[offset..offset + 2 * width].copy_from_slice(&src_b[..2 * width]),
            CaptureSource::AB => for (i, a_pixel) in src_a.iter().enumerate() {
                let b_pixel = HW::read_mem::<u16>(&src_b, i as u32 * 2);
                let a_alpha = a_pixel >> 15 & 0x1;
                let b_alpha = b_pixel >> 15 & 0x1;
                let mut intensity = 0;
                // TODO: Move blending into a utility function
                for i in (0..3).rev() {
                    let val_a = a_pixel >> (5 * i) & 0x1F;
                    let val_b = b_pixel >> (5 * i) & 0x1F;
                    let new_val = (val_a * a_alpha * self.dispcapcnt.eva as u16 +
                        val_b * b_alpha * self.dispcapcnt.evb as u16) / 16;
                    intensity = intensity << 5 | new_val;
                }
                let alpha = a_alpha != 0 && self.dispcapcnt.eva > 0 ||
                    b_alpha != 0 && self.dispcapcnt.evb > 0;
                let final_pixel = (alpha as u16) << 15 | intensity;
                HW::write_mem(bank, offset as u32 + 2 * i as u32, final_pixel);
            },
        }
    }

    pub fn bus_stalled(&self) -> bool {
        self.engine3d.bus_stalled
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

impl HW {
    pub fn start_next_line(&mut self, _event: Event) {
        self.scheduler.schedule(Event::HBlank, GPU::HBLANK_DOT * GPU::CYCLES_PER_DOT);
        self.gpu.start_next_line();
        if self.gpu.vcount == 0 {
            self.gpu.capturing = self.gpu.dispcapcnt.enable;
            for dispstat in self.gpu.dispstats.iter_mut() { dispstat.remove(DISPSTATFlags::VBLANK) }
        } else if self.gpu.vcount == GPU::HEIGHT as u16 {
            if self.gpu.capturing { self.gpu.dispcapcnt.enable = false }
            for dispstat in self.gpu.dispstats.iter_mut() { dispstat.insert(DISPSTATFlags::VBLANK) }
            self.gpu.rendered_frame = true;
            
            self.handle_event(Event::VBlank);
            self.check_dispstats(&mut |dispstat, interrupts|
                if dispstat.contains(DISPSTATFlags::VBLANK_IRQ_ENABLE) {
                    interrupts.request |= InterruptRequest::VBLANK;
                }
            );
        }

        let vcount = self.gpu.vcount;
        self.check_dispstats(&mut |dispstat, interrupts|
            if dispstat.contains(DISPSTATFlags::VBLANK_IRQ_ENABLE) && vcount == dispstat.vcount_setting {
                interrupts.request |= InterruptRequest::VCOUNTER_MATCH;
            }
        );
    }

    pub fn on_hblank(&mut self, _event: Event) {
        self.scheduler.schedule(Event::StartNextLine, (GPU::DOTS_PER_LINE - GPU::HBLANK_DOT) * GPU::CYCLES_PER_DOT);
        for dispstat in self.gpu.dispstats.iter_mut() { dispstat.insert(DISPSTATFlags::HBLANK) }
        if self.gpu.vcount < GPU::HEIGHT as u16 {
            self.gpu.render_line();
            self.run_dmas(DMAOccasion::HBlank);
        }
        self.check_dispstats(&mut |dispstat, interrupts|
            if dispstat.contains(DISPSTATFlags::HBLANK_IRQ_ENABLE) {
                interrupts.request |= InterruptRequest::HBLANK;
            }
        );
    }

    pub fn on_vblank(&mut self, _event: Event) {
        self.run_dmas(DMAOccasion::VBlank);
        // TODO: Render using multiple threads
        if self.gpu.powcnt1.contains(POWCNT1::ENABLE_3D_RENDERING) {
            self.gpu.engine3d.render(&self.gpu.vram)
        }
    }

    pub fn check_dispstats<F>(&mut self, check: &mut F) where F: FnMut(&mut DISPSTAT, &mut InterruptController) {
        for i in 0..2 { check(&mut self.gpu.dispstats[i], &mut self.interrupts[i]) }
    }
}

pub trait EngineType {
    fn is_a() -> bool;
}

pub struct EngineA {}
pub struct EngineB {}

impl EngineType for EngineA { fn is_a() -> bool { true }}
impl EngineType for EngineB { fn is_a() -> bool { false }}
