mod io;

use super::{AccessType, IORegister, MemoryValue, HW};
use crate::num;
use std::mem::size_of;

type MemoryRegion = ARM7MemoryRegion;

impl HW {
    pub fn arm7_read<T: MemoryValue>(&mut self, addr: u32) -> T {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::BIOS => HW::read_mem(&self.bios7, addr),
            MemoryRegion::MainMem => HW::read_mem(&self.main_mem, addr & HW::MAIN_MEM_MASK),
            MemoryRegion::SharedWRAM if self.wramcnt.arm7_mask == 0 => {
                warn!("Reading from Unmapped ARM7 Shared WRAM: 0x{:X}", addr);
                HW::read_mem(&self.iwram, addr & HW::IWRAM_MASK)
            }
            MemoryRegion::SharedWRAM => HW::read_mem(
                &self.shared_wram,
                self.wramcnt.arm7_offset + (addr & self.wramcnt.arm7_mask),
            ),
            MemoryRegion::IWRAM => HW::read_mem(&self.iwram, addr & HW::IWRAM_MASK),
            MemoryRegion::IO => self.arm7_read_io(addr),
            MemoryRegion::VRAM => self.gpu.vram.arm7_read(addr),
            MemoryRegion::GBAROM => self.read_gba_rom(false, addr),
            MemoryRegion::GBARAM => todo!(),
        }
    }

    pub fn arm7_write<T: MemoryValue>(&mut self, addr: u32, value: T) {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::BIOS => warn!("Writing to BIOS7 0x{:08x} = 0x{:X}", addr, value),
            MemoryRegion::MainMem => {
                HW::write_mem(&mut self.main_mem, addr & HW::MAIN_MEM_MASK, value)
            }
            MemoryRegion::SharedWRAM if self.wramcnt.arm7_mask == 0 => {
                HW::write_mem(&mut self.iwram, addr & HW::IWRAM_MASK, value)
            }
            MemoryRegion::SharedWRAM => HW::write_mem(
                &mut self.shared_wram,
                self.wramcnt.arm7_offset + addr & self.wramcnt.arm7_mask,
                value,
            ),
            MemoryRegion::IWRAM => HW::write_mem(&mut self.iwram, addr & HW::IWRAM_MASK, value),
            MemoryRegion::IO => self.arm7_write_io(addr, value),
            MemoryRegion::VRAM => self.gpu.vram.arm7_write(addr, value),
            MemoryRegion::GBAROM => (),
            MemoryRegion::GBARAM => todo!(),
        }
    }

    fn arm7_read_io<T: MemoryValue>(&mut self, addr: u32) -> T {
        match size_of::<T>() {
            1 => num::cast::<u8, T>(self.arm7_read_io8(addr)).unwrap(),
            2 => num::cast::<u16, T>(self.arm7_read_io16(addr)).unwrap(),
            4 => num::cast::<u32, T>(self.arm7_read_io32(addr)).unwrap(),
            _ => unreachable!(),
        }
    }

    fn arm7_write_io<T: MemoryValue>(&mut self, addr: u32, value: T) {
        match size_of::<T>() {
            1 => self.arm7_write_io8(addr, num::cast::<T, u8>(value).unwrap()),
            2 => self.arm7_write_io16(addr, num::cast::<T, u16>(value).unwrap()),
            4 => self.arm7_write_io32(addr, num::cast::<T, u32>(value).unwrap()),
            _ => unreachable!(),
        }
    }

    pub fn arm7_get_access_time<T: MemoryValue>(
        &mut self,
        _access_type: AccessType,
        _addr: u32,
    ) -> usize {
        // TODO: Use accurate timings
        1
    }

    pub fn init_arm7(&mut self) -> u32 {
        let start_addr = self.cartridge.header().arm7_ram_addr;
        let rom_offset = self.cartridge.header().arm7_rom_offset as usize;
        let size = self.cartridge.header().arm7_size;
        for (i, addr) in (start_addr..start_addr + size).enumerate() {
            self.arm7_write(addr, self.cartridge.rom()[rom_offset + i]);
        }
        self.cartridge.header().arm7_entry_addr
    }
}

pub enum ARM7MemoryRegion {
    BIOS,
    MainMem,
    SharedWRAM,
    IWRAM,
    IO,
    VRAM,
    GBAROM,
    GBARAM,
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
            0x6 => VRAM,
            0x8 | 0x9 => GBAROM,
            0xA => GBARAM,
            _ => todo!(),
        }
    }
}
