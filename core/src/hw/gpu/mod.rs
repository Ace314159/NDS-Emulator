mod registers;
mod engine2d;

use engine2d::Engine2D;

pub struct GPU {
    engineA: Engine2D,
}

impl GPU {
    pub fn new() -> GPU {
        GPU {
            engineA: Engine2D::new(),
        }
    }
}

