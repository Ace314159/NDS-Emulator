use super::{AccessType, HW, MemoryValue};

type MemoryRegion = ARM7MemoryRegion;

impl HW {
    pub fn arm7_read<T: MemoryValue>(&self, addr: u32) -> T {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::BIOS => HW::read_mem(&self.bios7, addr),
            MemoryRegion::MainMem => HW::read_mem(&self.main_mem, addr & HW::MAIN_MEM_MASK),
            MemoryRegion::SharedWRAM => todo!(),
            MemoryRegion::IWRAM => HW::read_mem(&self.iwram, addr & HW::IWRAM_MASK),
            MemoryRegion::IO => todo!(), 
        }
    }

    pub fn arm7_write<T: MemoryValue>(&mut self, addr: u32, value: T) {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::BIOS => warn!("Writing to BIOS7 0x{:08x} = 0x{:X}", addr, value),
            MemoryRegion::MainMem => HW::write_mem(&mut self.main_mem, addr & HW::MAIN_MEM_MASK, value),
            MemoryRegion::SharedWRAM => todo!(),
            MemoryRegion::IWRAM => HW::write_mem(&mut self.iwram, addr & HW::IWRAM_MASK, value),
            MemoryRegion::IO => todo!(), 
        }
    }

    pub fn arm7_get_access_time<T: MemoryValue>(&mut self, _access_type: AccessType, _addr: u32) -> usize {
        // TODO: Use accurate timings
        1
    }

    pub fn init_arm7(&mut self) -> u32 {
        let entry_region = MemoryRegion::from_addr(self.rom_header.arm7_entry_addr);
        let addr = self.rom_header.arm7_entry_addr as usize;
        let rom_offset = self.rom_header.arm7_rom_offset as usize;
        let size = self.rom_header.arm7_size as usize;
        match entry_region {
            MemoryRegion::MainMem => {
                let addr = addr & HW::MAIN_MEM_MASK as usize;
                self.main_mem[addr..addr + size].copy_from_slice(&self.rom[rom_offset..rom_offset + size])
            },
            MemoryRegion::IWRAM => {
                let addr = addr & HW::IWRAM_MASK as usize;
                self.iwram[addr..addr + size].copy_from_slice(&self.rom[rom_offset..rom_offset + size])
            },
            _ => panic!("Invalid ARM7 Entry Address: 0x{:08X}", self.rom_header.arm7_entry_addr),
        };
        self.rom_header.arm7_entry_addr
    }
}

pub enum ARM7MemoryRegion {
    BIOS,
    MainMem,
    SharedWRAM,
    IWRAM,
    IO,
}

impl ARM7MemoryRegion {
    pub fn from_addr(addr: u32) -> Self {
        use ARM7MemoryRegion::*;
        match addr >> 24 {
            0x0 => BIOS,
            0x2 => MainMem,
            0x3 if addr < 0x0380_0000 => SharedWRAM,
            0x3 => IWRAM,
            0x4 => IO,
            _ => todo!(),
        }
    }
}
