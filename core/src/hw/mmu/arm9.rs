use super::{AccessType, HW, MemoryValue};

type MemoryRegion = ARM9MemoryRegion;

impl HW {
    pub fn arm9_read<T: MemoryValue>(&self, addr: u32) -> T {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::MainMem => todo!(),
            MemoryRegion::WRAM => todo!(),
            MemoryRegion::SharedWRAM => todo!(),
            MemoryRegion::IO => todo!(),
            MemoryRegion::Palette => todo!(),
            MemoryRegion::VRAM => todo!(),
            MemoryRegion::OAM => todo!(),
            MemoryRegion::GBAROM => todo!(),
            MemoryRegion::GBARAM => todo!(),
            MemoryRegion::BIOS => todo!(),
        }
    }

    pub fn arm9_write<T: MemoryValue>(&mut self, addr: u32, value: T) {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::MainMem => todo!(),
            MemoryRegion::WRAM => todo!(),
            MemoryRegion::SharedWRAM => todo!(),
            MemoryRegion::IO => todo!(),
            MemoryRegion::Palette => todo!(),
            MemoryRegion::VRAM => todo!(),
            MemoryRegion::OAM => todo!(),
            MemoryRegion::GBAROM => todo!(),
            MemoryRegion::GBARAM => todo!(),
            MemoryRegion::BIOS => todo!(),
        }
    }

    pub fn arm9_get_access_time<T: MemoryValue>(&mut self, _access_type: AccessType, _addr: u32) -> usize {
        // TODO: Use accurate timings
        1
    }
}

pub enum ARM9MemoryRegion {
    MainMem,
    WRAM,
    SharedWRAM,
    IO,
    Palette,
    VRAM,
    OAM,
    GBAROM,
    GBARAM,
    BIOS,
}

impl ARM9MemoryRegion {
    pub fn from_addr(addr: u32) -> Self {
        use ARM9MemoryRegion::*;
        match addr >> 24 {
            0x2 => MainMem,
            0x3 => WRAM,
            0x4 => IO,
            0x5 => Palette,
            0x6 => VRAM,
            0x7 => OAM,
            0x8 | 0x9 => GBAROM,
            0xA => GBARAM,
            0xF if addr >> 16 == 0xFFFF => BIOS,
            _ => todo!(),
        }
    }
}
