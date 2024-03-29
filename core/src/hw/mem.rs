pub mod arm7;
pub mod arm9;
pub mod cp15;

use super::{Scheduler, HW};
use crate::num::{self, cast::FromPrimitive, NumCast, PrimInt, Unsigned};
pub use cp15::CP15;
use std::mem::size_of;
use std::ops::BitOrAssign;

impl HW {
    const IWRAM_MASK: u32 = HW::IWRAM_SIZE as u32 - 1;

    // TODO: Replace with const generic
    fn ipc_fifo_recv(&mut self, is_arm9: bool) -> u32 {
        if is_arm9 {
            let (value, interrupt) = self.ipc.arm9_recv();
            self.interrupts[0].request |= interrupt;
            value
        } else {
            let (value, interrupt) = self.ipc.arm7_recv();
            self.interrupts[1].request |= interrupt;
            value
        }
    }

    fn ipc_fifo_send(&mut self, is_arm9: bool, value: u32) {
        if is_arm9 {
            self.interrupts[1].request |= self.ipc.arm7_send(value);
        } else {
            self.interrupts[0].request |= self.ipc.arm9_send(value);
        }
    }

    fn read_game_card(&mut self, is_arm9: bool) -> u32 {
        let value = self.cartridge.read_gamecard(
            &mut self.scheduler,
            is_arm9,
            self.exmem.nds_arm7_access != is_arm9,
        );
        value
    }

    // TODO: Replace with const generic
    fn read_gba_rom<T: MemoryValue>(&self, is_arm9: bool, addr: u32) -> T {
        if self.exmem.gba_arm7_access != is_arm9 {
            let cnt = &self.exmem.gba[is_arm9 as usize];
            let value = match cnt.rom_n_access_time {
                0 => addr / 2 | 0xFE08,
                1 | 2 => addr / 2,
                3 => 0xFFFF,
                _ => unreachable!(),
            } & 0xFFFF;
            num::cast::<u32, T>(match size_of::<T>() {
                1 => value & 0xFF,
                2 => value,
                4 => (self.read_gba_rom::<u16>(is_arm9, addr + 1) as u32) << 16 | value,
                _ => unreachable!(),
            })
            .unwrap()
        } else {
            num::zero()
        }
    }

    pub(super) fn read_mem<T: MemoryValue>(mem: &[u8], addr: u32) -> T {
        unsafe { *(&mem[addr as usize] as *const u8 as *const T) }
    }

    pub(super) fn write_mem<T: MemoryValue>(mem: &mut [u8], addr: u32, value: T) {
        unsafe {
            *(&mut mem[addr as usize] as *mut u8 as *mut T) = value;
        }
    }

    fn read_from_bytes<T: MemoryValue, F: Fn(&D, u32) -> u8, D>(
        device: &D,
        read_fn: &F,
        addr: u32,
    ) -> T {
        let mut value: T = num::zero();
        for i in 0..(size_of::<T>() as u32) {
            value =
                num::cast::<u8, T>(read_fn(device, addr + i)).unwrap() << (8 * i as usize) | value;
        }
        value
    }

    fn _write_from_bytes<T: MemoryValue, F: Fn(&mut D, u32, u8), D>(
        device: &mut D,
        write_fn: &F,
        addr: u32,
        value: T,
    ) {
        let mask = FromPrimitive::from_u8(0xFF).unwrap();
        for i in 0..size_of::<T>() {
            write_fn(
                device,
                addr + i as u32,
                num::cast::<T, u8>(value >> 8 * i & mask).unwrap(),
            );
        }
    }

    pub fn read_byte_from_value<T: MemoryValue>(value: &T, byte: usize) -> u8 {
        let mask = FromPrimitive::from_u8(0xFF).unwrap();
        num::cast::<T, u8>((*value >> (byte * 8)) & mask).unwrap()
    }

    pub fn write_byte_to_value<T: MemoryValue>(value: &mut T, byte: usize, new_value: u8) {
        let mask: T = FromPrimitive::from_u64(0xFF << (8 * byte)).unwrap();
        let new_value: T = FromPrimitive::from_u8(new_value).unwrap();
        *value = *value & !mask | (new_value) << (8 * byte);
    }
}

pub trait MemoryValue:
    Unsigned + PrimInt + NumCast + FromPrimitive + std::fmt::UpperHex + BitOrAssign
{
}

impl MemoryValue for u8 {}
impl MemoryValue for u16 {}
impl MemoryValue for u32 {}
impl MemoryValue for u64 {}

#[derive(Clone, Copy)]
pub enum AccessType {
    N,
    S,
}

pub trait IORegister {
    fn read(&self, byte: usize) -> u8;
    fn write(&mut self, scheduler: &mut Scheduler, byte: usize, value: u8);
}

pub struct EXMEM {
    gba: [ExMemGBA; 2],
    gba_arm7_access: bool,
    nds_arm7_access: bool,
    main_mem_interface_mode: bool,
    main_mem_arm7_priority: bool,
}

impl EXMEM {
    pub fn new() -> Self {
        EXMEM {
            gba: [ExMemGBA::new(), ExMemGBA::new()],
            gba_arm7_access: false,
            nds_arm7_access: false,
            main_mem_interface_mode: true,
            main_mem_arm7_priority: false,
        }
    }

    pub fn read_arm7(&self) -> u8 {
        (self.gba_arm7_access as u8) << 7 | self.gba[0].read()
    }
    pub fn read_arm9(&self) -> u8 {
        (self.gba_arm7_access as u8) << 7 | self.gba[1].read()
    }
    pub fn read_common(&self) -> u8 {
        // TODO: Is bit 5 set or clear?
        (self.main_mem_arm7_priority as u8) << 7
            | (self.main_mem_interface_mode as u8) << 6
            | (self.nds_arm7_access as u8) << 3
    }
    pub fn write_arm7(&mut self, value: u8) {
        self.gba[0].write(value)
    }
    pub fn write_arm9(&mut self, value: u8) {
        self.gba_arm7_access = value >> 7 & 0x1 != 0;
        self.gba[1].write(value)
    }
    pub fn write_common(&mut self, value: u8) {
        self.main_mem_arm7_priority = value >> 7 & 0x1 != 0;
        self.main_mem_interface_mode = value >> 6 & 0x1 != 0;
        self.nds_arm7_access = value >> 3 & 0x1 != 0;
    }
}

pub struct ExMemGBA {
    sram_access_time: u8,
    rom_n_access_time: u8,
    rom_s_access_time: u8,
    phi: u8,
}

impl ExMemGBA {
    pub fn new() -> Self {
        ExMemGBA {
            sram_access_time: 0,
            rom_n_access_time: 0,
            rom_s_access_time: 0,
            phi: 0,
        }
    }

    pub fn read(&self) -> u8 {
        self.phi << 5
            | self.rom_s_access_time << 4
            | self.rom_n_access_time << 2
            | self.sram_access_time
    }

    pub fn write(&mut self, value: u8) {
        self.sram_access_time = value & 0x3;
        self.rom_n_access_time = value >> 2 & 0x3;
        self.rom_s_access_time = value >> 4 & 0x1;
        self.phi = value >> 5 & 0x3;
    }
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
                self.arm7_mask = 0;
                self.arm9_offset = 0;
                self.arm9_mask = HW::SHARED_WRAM_SIZE as u32 - 1;
            }
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
            }
            3 => {
                self.arm7_offset = 0;
                self.arm7_mask = HW::SHARED_WRAM_SIZE as u32 - 1;
                self.arm9_mask = 0;
            }
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

pub struct POWCNT2 {
    enable_sound: bool,
    enable_wifi: bool,
}

impl POWCNT2 {
    pub fn new() -> Self {
        POWCNT2 {
            enable_sound: true,
            enable_wifi: false,
        }
    }
}

impl IORegister for POWCNT2 {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.enable_wifi as u8) << 1 | (self.enable_sound as u8),
            1..=3 => 0,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => {
                self.enable_sound = value & 0x1 != 0;
                self.enable_wifi = value >> 1 & 0x1 != 0;
            }
            1..=3 => (),
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum HaltMode {
    None = 0,
    GBA = 1,
    Halt = 2,
    Sleep = 3,
}

impl HaltMode {
    fn from_bits(value: u8) -> Self {
        match value {
            0 => HaltMode::None,
            1 => HaltMode::GBA,
            2 => HaltMode::Halt,
            3 => HaltMode::Sleep,
            _ => unreachable!(),
        }
    }
}

pub struct HALTCNT {
    mode: HaltMode,
}

impl HALTCNT {
    pub fn new() -> Self {
        HALTCNT {
            mode: HaltMode::None,
        }
    }

    pub fn unhalt(&mut self) {
        self.mode = HaltMode::None;
    }
    pub fn halted(&self) -> bool {
        self.mode == HaltMode::Halt
    }
}

impl IORegister for HALTCNT {
    fn read(&self, byte: usize) -> u8 {
        assert_eq!(byte, 0);
        (self.mode as u8) << 6
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        assert_eq!(byte, 0);
        self.mode = HaltMode::from_bits(value >> 6);
        assert!(self.mode != HaltMode::GBA && self.mode != HaltMode::Sleep); // TODO: Implement
    }
}
