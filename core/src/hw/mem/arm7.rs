mod io;

use super::{AccessType, IORegister, MemoryValue, HW};
use crate::num;
use std::mem::size_of;

type MemoryRegion = ARM7MemoryRegion;

impl HW {
    const ARM7_PAGE_SHIFT: usize = 14;
    pub(in crate::hw) const ARM7_PAGE_TABLE_SIZE: usize = 1 << (32 - HW::ARM7_PAGE_SHIFT + 1);
    pub const ARM7_PAGE_SIZE: usize = 1 << HW::ARM7_PAGE_SHIFT;
    const ARM7_PAGE_TABLE_MASK: u32 = (HW::ARM7_PAGE_SIZE as u32) - 1;

    pub fn arm7_read<T: MemoryValue>(&mut self, addr: u32) -> T {
        let page_table_ptr = self.arm7_page_table[addr as usize >> HW::ARM7_PAGE_SHIFT];
        if !page_table_ptr.is_null() {
            unsafe {
                let slice = std::slice::from_raw_parts(page_table_ptr, HW::ARM7_PAGE_SIZE);
                HW::read_mem(slice, addr & HW::ARM7_PAGE_TABLE_MASK)
            }
        } else {
            match MemoryRegion::from_addr(addr) {
                MemoryRegion::SharedWRAM if self.wramcnt.arm7_mask == 0 => {
                    warn!("Reading from Unmapped ARM7 Shared WRAM: 0x{:X}", addr);
                    HW::read_mem(&self.iwram, addr & HW::IWRAM_MASK)
                }
                MemoryRegion::SharedWRAM => HW::read_mem(
                    &self.shared_wram,
                    self.wramcnt.arm7_offset + (addr & self.wramcnt.arm7_mask),
                ),
                MemoryRegion::IO => self.arm7_read_io(addr),
                MemoryRegion::VRAM => self.gpu.vram.arm7_read(addr),
                MemoryRegion::GBAROM => self.read_gba_rom(false, addr),
                MemoryRegion::GBARAM => todo!(),
            }
        }
    }

    pub fn arm7_write<T: MemoryValue>(&mut self, addr: u32, value: T) {
        let page_table_ptr = self.arm7_page_table[addr as usize >> HW::ARM7_PAGE_SHIFT];
        if !page_table_ptr.is_null() {
            if addr < self.bios7.len() as u32 {
                warn!("Writing to BIOS7 0x{:08x} = 0x{:X}", addr, value);
                return;
            }
            unsafe {
                let slice = std::slice::from_raw_parts_mut(page_table_ptr, HW::ARM7_PAGE_SIZE);
                HW::write_mem(slice, addr & HW::ARM7_PAGE_TABLE_MASK, value);
            }
        } else {
            match MemoryRegion::from_addr(addr) {
                MemoryRegion::SharedWRAM if self.wramcnt.arm7_mask == 0 => {
                    HW::write_mem(&mut self.iwram, addr & HW::IWRAM_MASK, value)
                }
                MemoryRegion::SharedWRAM => HW::write_mem(
                    &mut self.shared_wram,
                    self.wramcnt.arm7_offset + addr & self.wramcnt.arm7_mask,
                    value,
                ),
                MemoryRegion::IO => self.arm7_write_io(addr, value),
                MemoryRegion::VRAM => self.gpu.vram.arm7_write(addr, value),
                MemoryRegion::GBAROM => (),
                MemoryRegion::GBARAM => todo!(),
            }
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

    pub fn init_arm7_page_tables(&mut self) {
        Self::map_page_table(
            &mut self.arm7_page_table,
            HW::ARM7_PAGE_SHIFT,
            HW::ARM7_PAGE_SIZE,
            0x00000000,
            self.bios7.len(),
            &mut self.bios7,
        );
        Self::map_page_table(
            &mut self.arm7_page_table,
            HW::ARM7_PAGE_SHIFT,
            HW::ARM7_PAGE_SIZE,
            0x02000000,
            0x03000000,
            &mut self.main_mem,
        );
        Self::map_page_table(
            &mut self.arm7_page_table,
            HW::ARM7_PAGE_SHIFT,
            HW::ARM7_PAGE_SIZE,
            0x03800000,
            0x04000000,
            &mut self.iwram,
        );
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
    SharedWRAM,
    IO,
    VRAM,
    GBAROM,
    GBARAM,
}

impl ARM7MemoryRegion {
    pub fn from_addr(addr: u32) -> Self {
        use ARM7MemoryRegion::*;
        match addr >> 24 {
            0x3 => SharedWRAM, // Other have 0f 0x3 is accounted by fastmem
            0x4 => IO,
            0x6 => VRAM,
            0x8 | 0x9 => GBAROM,
            0xA => GBARAM,
            _ => todo!(),
        }
    }
}
