mod registers;
mod engine2d;
mod vram;

use crate::hw::interrupt_controller::InterruptRequest;
pub use engine2d::Engine2D;
use vram::VRAM;

pub struct GPU {
    pub engine_a: Engine2D,
    pub engine_b: Engine2D,
    pub vram: VRAM,
}

impl GPU {
    pub const WIDTH: usize = 256;
    pub const HEIGHT: usize = 192;

    pub fn new() -> GPU {
        GPU {
            engine_a: Engine2D::new(),
            engine_b: Engine2D::new(),
            vram: VRAM::new(),
        }
    }

    pub fn emulate_dot(&mut self) -> InterruptRequest {
        // TODO: Optimize
        let interrupts_a = self.engine_a.emulate_dot(&self.vram);
        let interrupts_b = self.engine_b.emulate_dot(&self.vram);
        assert_eq!(interrupts_a, interrupts_b);
        interrupts_a
    }

    pub fn rendered_frame(&mut self) -> bool {
        let result = self.engine_a.rendered_frame();
        assert_eq!(result, self.engine_b.rendered_frame());
        result
    }

    pub fn get_screens(&self) -> [&Vec<u16>; 2] {
        [&self.engine_b.pixels, &self.engine_a.pixels]
    }
}
