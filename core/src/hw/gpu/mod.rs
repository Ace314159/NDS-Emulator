mod registers;
mod engine2d;

pub use engine2d::Engine2D;
use crate::hw::interrupt_controller::InterruptRequest;

pub struct GPU {
    pub engine_a: Engine2D,
    pub engine_b: Engine2D,
}

impl GPU {
    pub fn new() -> GPU {
        GPU {
            engine_a: Engine2D::new(),
            engine_b: Engine2D::new(),
        }
    }

    pub fn emulate_dot(&mut self) -> InterruptRequest {
        // TODO: Optimize
        let interrupts_a = self.engine_a.emulate_dot();
        let interrupts_b = self.engine_b.emulate_dot();
        assert_eq!(interrupts_a, interrupts_b);
        interrupts_a
    }
}
