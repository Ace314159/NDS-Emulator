use super::{AccessType, HW, MemoryValue};

type MemoryRegion = ARM9MemoryRegion;

impl HW {
    pub fn arm9_read<T: MemoryValue>(&self, addr: u32) -> T {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::MainMem => HW::read_mem(&self.main_mem, addr & HW::MAIN_MEM_MASK),
            MemoryRegion::WRAM => todo!(),
            MemoryRegion::SharedWRAM => todo!(),
            MemoryRegion::IO => todo!(),
            MemoryRegion::Palette => todo!(),
            MemoryRegion::VRAM => todo!(),
            MemoryRegion::OAM => todo!(),
            MemoryRegion::GBAROM => todo!(),
            MemoryRegion::GBARAM => todo!(),
            MemoryRegion::BIOS => HW::read_mem(&self.bios9, addr),
        }
    }

    pub fn arm9_write<T: MemoryValue>(&mut self, addr: u32, value: T) {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::MainMem => HW::write_mem(&mut self.main_mem, addr & HW::MAIN_MEM_MASK, value),
            MemoryRegion::WRAM => todo!(),
            MemoryRegion::SharedWRAM => todo!(),
            MemoryRegion::IO => todo!(),
            MemoryRegion::Palette => todo!(),
            MemoryRegion::VRAM => todo!(),
            MemoryRegion::OAM => todo!(),
            MemoryRegion::GBAROM => todo!(),
            MemoryRegion::GBARAM => todo!(),
            MemoryRegion::BIOS => warn!("Writing to BIOS9 0x{:08x} = 0x{:X}", addr, value),
        }
    }

    pub fn arm9_get_access_time<T: MemoryValue>(&mut self, _access_type: AccessType, _addr: u32) -> usize {
        // TODO: Use accurate timings
        1
    }

    pub fn init_arm9(&mut self) -> u32 {
        let entry_region = MemoryRegion::from_addr(self.rom_header.arm9_entry_addr);
        let addr = self.rom_header.arm9_entry_addr as usize & 0xFF_FFFF;
        let rom_offset = self.rom_header.arm9_rom_offset as usize;
        let size = self.rom_header.arm9_size as usize;
        match entry_region {
            MemoryRegion::MainMem =>
                self.main_mem[addr..addr + size].copy_from_slice(&self.rom[rom_offset..rom_offset + size]),
            _ => panic!("Invalid ARM7 Entry Address: 0x{:08X}", self.rom_header.arm9_entry_addr),
        };
        self.rom_header.arm9_entry_addr
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
