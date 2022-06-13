use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

use crate::arm::ARM;
use crate::hw::HW;

pub use crate::hw::{Engine, GraphicsType, Key};

pub struct NDS {
    arm9_cycles_ahead: i32, // Measured in 66 MHz ARM9 cycles
    arm7: ARM<false>,
    arm9: ARM<true>,
    hw: HW,
}

impl NDS {
    pub const CLOCK_RATE: usize = 33513982;

    pub fn new(
        bios7: Vec<u8>,
        bios9: Vec<u8>,
        firmware_file: File,
        rom: Vec<u8>,
        save_file: File,
    ) -> Self {
        let direct_boot = true;
        let mut hw = HW::new(bios7, bios9, firmware_file, rom, save_file, direct_boot);
        NDS {
            arm9_cycles_ahead: 0,
            arm7: ARM::new(&mut hw, direct_boot),
            arm9: ARM::new(&mut hw, direct_boot),
            hw,
        }
    }

    pub fn emulate_frame(&mut self) {
        while !self.hw.rendered_frame() {
            if !self.hw.gpu.bus_stalled() {
                self.arm9.handle_irq(&mut self.hw);
                self.arm9_cycles_ahead += if self.hw.cp15.arm9_halted {
                    self.hw.cycles_until_event()
                } else {
                    self.arm9.emulate_instr(&mut self.hw)
                } as i32;

                while self.arm9_cycles_ahead >= 0 {
                    self.arm7.handle_irq(&mut self.hw);
                    let arm7_cycles_ran = if self.hw.haltcnt.halted() {
                        1
                    } else {
                        self.arm7.emulate_instr(&mut self.hw)
                    };
                    self.hw.clock(arm7_cycles_ran);
                    self.arm9_cycles_ahead -= 2 * arm7_cycles_ran as i32
                }
            } else {
                self.hw.clock_until_event()
            }
        }
    }

    #[inline]
    pub fn get_screens(&self) -> [&Vec<u16>; 2] {
        self.hw.gpu.get_screens()
    }

    #[inline]
    pub fn press_key(&mut self, key: Key) {
        self.hw.press_key(key);
    }

    #[inline]
    pub fn release_key(&mut self, key: Key) {
        self.hw.release_key(key);
    }

    #[inline]
    pub fn press_screen(&mut self, x: usize, y: usize) {
        self.hw.press_screen(x, y);
    }

    #[inline]
    pub fn release_screen(&mut self) {
        self.hw.release_screen();
    }

    #[inline]
    pub fn render_palettes(
        &self,
        extended: bool,
        slot: usize,
        palette: usize,
        engine: Engine,
        graphics_type: GraphicsType,
    ) -> (Vec<u16>, usize, usize) {
        self.hw
            .render_palettes(extended, slot, palette, engine, graphics_type)
    }

    #[inline]
    pub fn render_map(&self, engine: Engine, bg_i: usize) -> (Vec<u16>, usize, usize) {
        self.hw.render_map(engine, bg_i)
    }

    #[inline]
    pub fn render_tiles(
        &self,
        engine: Engine,
        graphics_type: GraphicsType,
        extended: bool,
        bitmap: bool,
        bpp8: bool,
        slot: usize,
        palette: usize,
        offset: usize,
    ) -> (Vec<u16>, usize, usize) {
        self.hw.render_tiles(
            engine,
            graphics_type,
            extended,
            bitmap,
            bpp8,
            slot,
            palette,
            offset,
        )
    }

    #[inline]
    pub fn render_bank(&self, bank: usize, ignore_alpha: bool) -> (Vec<u16>, usize, usize) {
        self.hw.render_bank(ignore_alpha, bank)
    }

    pub fn load_rom(
        bios7_path: &PathBuf,
        bios9_path: &PathBuf,
        firmware_path: &PathBuf,
        rom_path: &Path,
    ) -> Self {
        let save_file_path = rom_path.with_extension("sav");
        let save_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&save_file_path)
            .unwrap();
        let mut firmware_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&firmware_path)
            .unwrap();
        let firmware_bak = PathBuf::from(firmware_path.to_str().unwrap().to_owned() + ".bak");

        let mut firmware_bak_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&firmware_bak)
            .unwrap();
        if firmware_file.metadata().unwrap().len() != firmware_bak_file.metadata().unwrap().len() {
            std::io::copy(&mut firmware_file, &mut firmware_bak_file).unwrap();
        }

        NDS::new(
            fs::read(bios7_path).unwrap(),
            fs::read(bios9_path).unwrap(),
            firmware_file,
            fs::read(rom_path).unwrap(),
            save_file,
        )
    }
}

pub const WIDTH: usize = crate::hw::GPU::WIDTH;
pub const HEIGHT: usize = crate::hw::GPU::HEIGHT;
