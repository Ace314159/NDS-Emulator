pub mod arm7;
pub mod arm9;

use std::mem::size_of;
use crate::num::{self, cast::FromPrimitive, NumCast, PrimInt, Unsigned};
use super::{HW, Scheduler};

impl HW {
    const MAIN_MEM_MASK: u32 = HW::MAIN_MEM_SIZE as u32 - 1;
    const IWRAM_MASK: u32 = HW::IWRAM_SIZE as u32 - 1;

    fn read_mem<T: MemoryValue>(mem: &Vec<u8>, addr: u32) -> T {
        unsafe {
            *(&mem[addr as usize] as *const u8 as *const T)
        }
    }

    fn write_mem<T: MemoryValue>(mem: &mut Vec<u8>, addr: u32, value: T) {
        unsafe {
            *(&mut mem[addr as usize] as *mut u8 as *mut T) = value;
        }
    }

    fn read_from_bytes<T: MemoryValue, F: Fn(&D, u32) -> u8, D>(device: &D, read_fn: &F, addr: u32) -> T {
        let mut value: T = num::zero();
        for i in 0..(size_of::<T>() as u32) {
            value = num::cast::<u8, T>(read_fn(device, addr + i)).unwrap() << (8 * i as usize) | value;
        }
        value
    }

    fn write_from_bytes<T: MemoryValue, F: Fn(&mut D, u32, u8), D>(device: &mut D, write_fn: &F, addr: u32, value: T) {
        let mask = FromPrimitive::from_u8(0xFF).unwrap();
        for i in 0..size_of::<T>() {
            write_fn(device, addr + i as u32, num::cast::<T, u8>(value >> 8 * i & mask).unwrap());
        }
    }
}

pub trait MemoryValue: Unsigned + PrimInt + NumCast + FromPrimitive + std::fmt::UpperHex {}

impl MemoryValue for u8 {}
impl MemoryValue for u16 {}
impl MemoryValue for u32 {}

#[derive(Clone, Copy)]
pub enum AccessType {
    N,
    S,
}

pub trait IORegister {
    fn read(&self, byte: usize) -> u8;
    fn write(&mut self, scheduler: &mut Scheduler, byte: usize, value: u8);
}

pub struct WRAMCNT {
    value: u8,

    arm7_offset: u32,
    arm7_mask: u32,
    arm9_offset: u32,
    arm9_mask: u32,
}

impl WRAMCNT {
    pub fn new(value: u8) -> Self {
        let mut wramcnt = WRAMCNT {
            value,

            arm7_offset: 0,
            arm7_mask: 0,
            arm9_offset: 0,
            arm9_mask: 0,
        };
        wramcnt.changed();
        wramcnt
    }

    fn changed(&mut self) {
        match self.value {
            0 => {
                self.arm7_offset = 0;
                self.arm9_mask = 0;
                self.arm9_mask = HW::SHARED_WRAM_SIZE as u32 - 1;
            },
            1 => {
                self.arm7_offset = 0;
                self.arm7_mask = HW::SHARED_WRAM_SIZE as u32 / 2 - 1;
                self.arm9_offset = HW::SHARED_WRAM_SIZE as u32 / 2;
                self.arm9_mask = HW::SHARED_WRAM_SIZE as u32 / 2 - 1;
            }
            2 => {
                self.arm7_offset = HW::SHARED_WRAM_SIZE as u32 / 2;
                self.arm7_mask = HW::SHARED_WRAM_SIZE as u32 / 2 - 1;
                self.arm9_offset = 0;
                self.arm9_mask = HW::SHARED_WRAM_SIZE as u32 / 2 - 1;
            },
            3 => {
                self.arm7_mask = 0;
                self.arm9_offset = 0;
                self.arm9_mask = HW::SHARED_WRAM_SIZE as u32 - 1;
            },
            _ => unreachable!(),
        }
    }
}

impl IORegister for WRAMCNT {
    fn read(&self, byte: usize) -> u8 {
        assert_eq!(byte, 0);
        self.value
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        assert_eq!(byte, 0);
        self.value = value & 0x3;
        self.changed();
    }

}
