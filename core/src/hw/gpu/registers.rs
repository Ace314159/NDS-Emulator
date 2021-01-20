use std::ops::{Deref, DerefMut};

use bitflags::*;

use crate::hw::{
    HW,
    mem::IORegister,
    Scheduler
};

bitflags! {
    pub struct POWCNT1: u32 {
        const ENABLE_LCDS = 1 << 0;
        const ENABLE_ENGINE_A = 1 << 1;
        const ENABLE_3D_RENDERING = 1 << 2;
        const ENABLE_3D_GEOMETRY = 1 << 3; // TODO: Check what this affects
        const ENABLE_ENGINE_B = 1 << 9;
        const TOP_A = 1 << 15;
    }
}

impl IORegister for POWCNT1 {
    fn read(&self, byte: usize) -> u8 {
        assert!(byte < 4);
        HW::read_byte_from_value(&self.bits, byte)
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        assert!(byte < 4);
        HW::write_byte_to_value(&mut self.bits, byte, value);
        self.bits &= POWCNT1::all().bits;
        assert!(self.contains(POWCNT1::ENABLE_LCDS)); // TODO: Figure out what this does
    }
}

bitflags! {
    pub struct DISPSTATFlags: u16 {
        const VBLANK = 1 << 0;
        const HBLANK = 1 << 1;
        const VCOUNTER = 1 << 2;
        const VBLANK_IRQ_ENABLE = 1 << 3;
        const HBLANK_IRQ_ENABLE = 1 << 4;
        const VCOUNTER_IRQ_ENALBE = 1 << 5;
    }
}

pub struct DISPSTAT {
    pub flags: DISPSTATFlags,
    pub vcount_setting: u16,
}

impl DISPSTAT {
    pub fn new() -> DISPSTAT {
        DISPSTAT {
            flags: DISPSTATFlags::empty(),
            vcount_setting: 0,
        }
    }
}

impl Deref for DISPSTAT {
    type Target = DISPSTATFlags;

    fn deref(&self) -> &DISPSTATFlags {
        &self.flags
    }
}

impl DerefMut for DISPSTAT {
    fn deref_mut(&mut self) -> &mut DISPSTATFlags {
        &mut self.flags
    }
}

impl IORegister for DISPSTAT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.vcount_setting >> 1) as u8 & 0x80 | self.flags.bits as u8,
            1 => self.vcount_setting as u8,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => {
                let old_bits = self.flags.bits;
                self.flags.bits = self.flags.bits & 0x7 | ((value as u16) & !0x7 & DISPSTATFlags::all().bits);
                assert_eq!(old_bits & 0x7, self.flags.bits & 0x7);
                self.vcount_setting = self.vcount_setting & !0x100 | (value as u16 & 0x80) << 8;
            },
            1 => self.vcount_setting = self.vcount_setting & !0xFF | value as u16,
            _ => unreachable!(),
        }
    }
}

pub struct DISPCAPCNT {
    pub eva: u8,
    pub evb: u8,
    pub vram_write_block: usize,
    pub vram_write_offset: CaptureOffset,
    pub capture_size: CaptureSize,
    pub src_a_is_3d_only: bool,
    pub src_b_fifo: bool,
    pub vram_read_offset: CaptureOffset,
    pub capture_src: CaptureSource,
    pub enable: bool,
}

impl DISPCAPCNT {
    pub fn new() -> DISPCAPCNT {
        DISPCAPCNT {
            eva: 0,
            evb: 0,
            vram_write_block: 0,
            vram_write_offset: CaptureOffset::O00000,
            capture_size: CaptureSize::S128x128,
            src_a_is_3d_only: false,
            src_b_fifo: false,
            vram_read_offset: CaptureOffset::O00000,
            capture_src: CaptureSource::A,
            enable: false,
        }
    }
}

impl IORegister for DISPCAPCNT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => self.eva,
            1 => self.evb,
            2 => (self.capture_size as u8) << 4 | (self.vram_write_offset as u8) << 2 | self.vram_write_block as u8,
            3 => (self.enable as u8) << 7 | (self.capture_src as u8) << 5 | (self.vram_read_offset as u8) << 2 |
                (self.src_b_fifo as u8) << 1 | self.src_a_is_3d_only as u8,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => self.eva = value & 0x1F,
            1 => self.evb = value & 0x1F,
            2 => {
                self.vram_write_block = (value & 0x3) as usize;
                self.vram_write_offset = CaptureOffset::from(value >> 2 & 0x3);
                self.capture_size = CaptureSize::from(value >> 4 & 0x3);
            },
            3 => {
                self.src_a_is_3d_only = value & 0x1 != 0;
                self.src_b_fifo = value >> 1 & 0x1 != 0;
                self.vram_read_offset = CaptureOffset::from(value >> 2 & 0x3);
                self.capture_src = CaptureSource::from(value >> 5 & 0x3);
                self.enable = value >> 7 & 0x1 != 0;
            },
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy)]
pub enum CaptureOffset {
    O00000 = 0,
    O08000 = 1,
    O10000 = 2,
    O18000 = 3,
}

impl CaptureOffset {
    pub fn offset(&self) -> usize {
        match *self {
            CaptureOffset::O00000 => 0x00000,
            CaptureOffset::O08000 => 0x08000,
            CaptureOffset::O10000 => 0x10000,
            CaptureOffset::O18000 => 0x18000,
        }
    }
}

impl From<u8> for CaptureOffset {
    fn from(value: u8) -> Self {
        match value {
            0 => CaptureOffset::O00000,
            1 => CaptureOffset::O08000,
            2 => CaptureOffset::O10000,
            3 => CaptureOffset::O18000,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy)]
pub enum CaptureSize {
    S128x128 = 0,
    S256x64 = 1,
    S256x128 = 2,
    S256x192 = 3,
}

impl CaptureSize {
    pub fn width(&self) -> usize {
        match *self {
            CaptureSize::S128x128 => 128,
            CaptureSize::S256x64 => 256,
            CaptureSize::S256x128 => 256,
            CaptureSize::S256x192 => 256,
        }
    }

    pub fn height(&self) -> usize {
        match *self {
            CaptureSize::S128x128 => 128,
            CaptureSize::S256x64 => 64,
            CaptureSize::S256x128 => 128,
            CaptureSize::S256x192 => 192,
        }
    }
}

impl From<u8> for CaptureSize {
    fn from(value: u8) -> Self {
        match value {
            0 => CaptureSize::S128x128,
            1 => CaptureSize::S256x64,
            2 => CaptureSize::S256x128,
            3 => CaptureSize::S256x192,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum CaptureSource {
    A = 0,
    B = 1,
    AB = 2,
}

impl From<u8> for CaptureSource {
    fn from(value: u8) -> Self {
        match value {
            0 => CaptureSource::A,
            1 => CaptureSource::B,
            2 | 3 => CaptureSource::AB,
            _ => unreachable!(),
        }
    }
}
