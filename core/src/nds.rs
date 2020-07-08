use crate::arm7::ARM7;
use crate::arm9::ARM9;
use crate::hw::HW;

pub struct NDS {
    arm7: ARM7,
    arm9: ARM9,
    hw: HW,
}

impl NDS {
    pub fn new() -> Self {
        let mut hw = HW::new();
        NDS {
            arm7: ARM7::new(false, &mut hw),
            arm9: ARM9::new(false, &mut hw),
            hw,
        }
    }

    pub fn emulate_frame(&mut self) {
        self.arm7.emulate_instr(&mut self.hw);
    }
}
