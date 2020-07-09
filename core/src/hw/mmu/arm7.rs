use super::{AccessType, HW, MemoryValue};

type MemoryRegion = ARM7MemoryRegion;

impl HW {
    pub fn arm7_read<T: MemoryValue>(&self, addr: u32) -> T {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::BIOS => todo!(),
            MemoryRegion::MainMem => todo!(),
            MemoryRegion::WRAM => todo!(),
            MemoryRegion::IO => todo!(), 
        }
    }

    pub fn arm7_write<T: MemoryValue>(&mut self, addr: u32, value: T) {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::BIOS => todo!(),
            MemoryRegion::MainMem => todo!(),
            MemoryRegion::WRAM => todo!(),
            MemoryRegion::IO => todo!(), 
        }
    }

    pub fn arm7_get_access_time<T: MemoryValue>(&mut self, _access_type: AccessType, _addr: u32) -> usize {
        // TODO: Use accurate timings
        1
    }
}

pub enum ARM7MemoryRegion {
    BIOS,
    MainMem,
    WRAM,
    IO,
}

impl ARM7MemoryRegion {
    pub fn from_addr(addr: u32) -> Self {
        use ARM7MemoryRegion::*;
        match addr >> 24 {
            0x0 => BIOS,
            0x2 => MainMem,
            0x3 => WRAM,
            0x4 => IO,
            _ => todo!(),
        }
    }
}
