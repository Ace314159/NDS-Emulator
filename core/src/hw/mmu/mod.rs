pub mod arm7;
pub mod arm9;

use std::mem::size_of;
use crate::num::{self, cast::FromPrimitive, NumCast, PrimInt, Unsigned};
use super::HW;

impl HW {
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

pub trait MemoryValue: Unsigned + PrimInt + NumCast + FromPrimitive {}

impl MemoryValue for u8 {}
impl MemoryValue for u16 {}
impl MemoryValue for u32 {}

#[derive(Clone, Copy)]
pub enum AccessType {
    N,
    S,
}
