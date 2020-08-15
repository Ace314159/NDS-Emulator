use std::ops::{Deref, DerefMut};

use bitflags::*;

use crate::hw::{
    HW,
    mmu::IORegister,
    Scheduler
};

bitflags! {
    pub struct POWCNT1: u32 {
        const ENABLE_LCDS = 1 << 0;
        const ENABLE_ENGINE_A = 1 << 1;
        const ENABLE_3D_RENDERING = 1 << 2;
        const ENABLE_3D_GEOMETRY = 1 << 3;
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
