use crate::hw::HW;

pub struct ARM7 {

}

impl ARM7 {
    pub fn new() -> Self {
        ARM7 {

        }
    }

    pub fn emulate_instr(&mut self, hw: &mut HW) -> usize {
        0
    }
}
