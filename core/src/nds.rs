use crate::arm7::ARM7;
use crate::hw::HW;

pub struct NDS {
    arm7: ARM7,
    hw: HW,
}

impl NDS {
    pub fn new() -> Self {
        NDS {
            arm7: ARM7::new(),
            hw: HW::new(),
        }
    }

    pub fn emulate_frame(&mut self) {
        self.arm7.emulate_instr(&mut self.hw);
    }
}
