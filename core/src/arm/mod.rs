use crate::hw::HW;

pub struct ARM<const IS_ARM9: bool> {

}

impl<const IS_ARM9: bool> ARM<IS_ARM9> {
    pub fn new(_hw: &mut HW, _direct_boot: bool) -> Self {
        ARM {
            
        }
    }

    pub fn emulate_instr(&mut self, _hw: &mut HW) -> usize {
        0
    }

    pub fn handle_irq(&mut self, _hw: &mut HW) {
        
    }
}
