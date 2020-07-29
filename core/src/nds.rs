use crate::arm7::ARM7;
use crate::arm9::ARM9;
use crate::hw::HW;

pub use crate::hw::{
    Engine,
    GraphicsType,
    Key
};

pub struct NDS {
    arm9_cycles_ahead: i32, // Measured in 66 MHz ARM9 cycles
    arm7: ARM7,
    arm9: ARM9,
    hw: HW,
}

impl NDS {
    pub fn new(bios7: Vec<u8>, bios9: Vec<u8>, firmware: Vec<u8>, rom: Vec<u8>) -> Self {
        let mut hw = HW::new(bios7, bios9, firmware, rom);
        NDS {
            arm9_cycles_ahead: 0,
            arm7: ARM7::new(&mut hw),
            arm9: ARM9::new(&mut hw),
            hw,
        }
    }

    pub fn emulate_frame(&mut self) {
        while !self.hw.rendered_frame() {
            self.arm9.handle_irq(&mut self.hw);
            if !self.hw.cp15.arm9_halted {
                self.arm9_cycles_ahead += self.arm9.emulate_instr(&mut self.hw) as i32;
            }
            while self.arm9_cycles_ahead >= 0 || self.hw.cp15.arm9_halted {
                self.arm7.handle_irq(&mut self.hw);
                let arm7_cycles_ran = if self.hw.haltcnt.halted() { 1 }
                else { self.arm7.emulate_instr(&mut self.hw) };
                self.hw.clock(arm7_cycles_ran);
                if self.hw.cp15.arm9_halted { break }
                else { self.arm9_cycles_ahead -= 2 * arm7_cycles_ran as i32 }
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

    pub fn render_palettes(&self, extended: bool, slot: usize,
        engine: Engine, graphics_type: GraphicsType) -> (Vec<u16>, usize, usize) {
        self.hw.render_palettes(extended, slot, engine, graphics_type)
    }

    pub fn render_map(&self, engine: Engine, bg_i: usize) -> (Vec<u16>, usize, usize) {
        self.hw.render_map(engine, bg_i)
    }
}

pub const WIDTH: usize = crate::hw::GPU::WIDTH;
pub const HEIGHT: usize = crate::hw::GPU::HEIGHT;
