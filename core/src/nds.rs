use crate::arm7::ARM7;
use crate::arm9::ARM9;
use crate::hw::HW;

pub use crate::hw::Key;

pub struct NDS {
    arm9_cycles_ahead: i32, // Measured in 66 MHz ARM9 cycles
    arm7: ARM7,
    arm9: ARM9,
    hw: HW,
}

impl NDS {
    pub fn new(bios7: Vec<u8>, bios9: Vec<u8>, rom: Vec<u8>) -> Self {
        let mut hw = HW::new(bios7, bios9, rom);
        NDS {
            arm9_cycles_ahead: 0,
            arm7: ARM7::new(&mut hw),
            arm9: ARM9::new(&mut hw),
            hw,
        }
    }

    pub fn emulate_frame(&mut self) {
        while !self.hw.rendered_frame() {
            self.arm7.handle_irq(&mut self.hw);
            self.arm9_cycles_ahead += 2 * self.arm7.emulate_instr(&mut self.hw) as i32;
            self.hw.clock();
            while self.arm9_cycles_ahead >= 0 {
                self.arm9.handle_irq(&mut self.hw);
                self.arm9_cycles_ahead -= self.arm9.emulate_instr(&mut self.hw) as i32;
            }
        }
    }

    pub fn get_screens(&self) -> [&Vec<u16>; 2] {
        self.hw.gpu.get_screens()
    }

    pub fn press_key(&mut self, key: Key) {
        self.hw.press_key(key);
    }

    pub fn release_key(&mut self, key: Key) {
        self.hw.release_key(key);
    }
}

pub const WIDTH: usize = crate::hw::GPU::WIDTH;
pub const HEIGHT: usize = crate::hw::GPU::HEIGHT;
