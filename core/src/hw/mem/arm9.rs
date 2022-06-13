mod io;

use super::{AccessType, IORegister, MemoryValue, HW};
use crate::hw::gpu::{Engine2D, EngineType, GPU};
use crate::{num, unlikely};
use std::mem::size_of;

type MemoryRegion = ARM9MemoryRegion;

impl HW {
    const ARM9_PAGE_SHIFT: usize = 12;
    pub(in crate::hw) const ARM9_PAGE_TABLE_SIZE: usize = 1 << (32 - HW::ARM9_PAGE_SHIFT + 1);
    pub const ARM9_PAGE_SIZE: usize = 1 << HW::ARM9_PAGE_SHIFT;
    const ARM9_PAGE_TABLE_MASK: u32 = (HW::ARM9_PAGE_SIZE as u32) - 1;

    pub fn arm9_read<T: MemoryValue>(&mut self, addr: u32) -> T {
        let page_table_ptr = self.arm9_page_table[addr as usize >> HW::ARM9_PAGE_SHIFT];
        if !page_table_ptr.is_null() {
            unsafe {
                let slice = std::slice::from_raw_parts(page_table_ptr, HW::ARM9_PAGE_SIZE);
                HW::read_mem(slice, addr & HW::ARM9_PAGE_TABLE_MASK)
            }
        } else {
            match MemoryRegion::from_addr(addr) {
                MemoryRegion::SharedWRAM if self.wramcnt.arm9_mask == 0 => {
                    warn!("Reading from Unmapped ARM9 Shared WRAM: 0x{:X}", addr);
                    num::zero()
                }
                MemoryRegion::SharedWRAM => HW::read_mem(
                    &self.shared_wram,
                    self.wramcnt.arm9_offset + (addr & self.wramcnt.arm9_mask),
                ),
                MemoryRegion::IO => self.arm9_read_io(addr),
                MemoryRegion::Palette if addr & 0x7FFF < 0x400 => {
                    HW::read_from_bytes(&self.gpu.engine_a, &Engine2D::read_palette_ram, addr as u32)
                }
                MemoryRegion::Palette => {
                    HW::read_from_bytes(&self.gpu.engine_b, &Engine2D::read_palette_ram, addr as u32)
                }
                MemoryRegion::VRAM => self.gpu.vram.arm9_read(addr),
                MemoryRegion::OAM if addr & 0x7FFF < 0x400 => {
                    HW::read_mem(&self.gpu.engine_a.oam, addr & GPU::OAM_MASK as u32)
                }
                MemoryRegion::OAM => HW::read_mem(&self.gpu.engine_b.oam, addr & GPU::OAM_MASK as u32),
                MemoryRegion::GBAROM => self.read_gba_rom(true, addr),
                MemoryRegion::GBARAM => todo!(),
                MemoryRegion::Unknown => {
                    warn!("Reading from Unknown 0x{:08X}", addr);
                    num::zero()
                }
            }
        }
    }

    pub fn arm9_write<T: MemoryValue>(&mut self, addr: u32, value: T) {
        let page_table_ptr = self.arm9_page_table[addr as usize >> HW::ARM9_PAGE_SHIFT];
        if !page_table_ptr.is_null() {
            if unlikely(addr >> 16 == 0xFFFF) {
                warn!("Writing to BIOS9 0x{:08x} = 0x{:X}", addr, value);
                return;
            }
            unsafe {
                let slice = std::slice::from_raw_parts_mut(page_table_ptr, HW::ARM9_PAGE_SIZE);
                HW::write_mem(slice, addr & HW::ARM9_PAGE_TABLE_MASK, value);
            }
        } else {
            match MemoryRegion::from_addr(addr) {
                MemoryRegion::SharedWRAM if self.wramcnt.arm9_mask == 0 => {
                    warn!("Writing to Unmapped ARM9 Shared WRAM")
                }
                MemoryRegion::SharedWRAM => HW::write_mem(
                    &mut self.shared_wram,
                    self.wramcnt.arm9_offset + addr & self.wramcnt.arm9_mask,
                    value,
                ),
                MemoryRegion::IO => self.arm9_write_io(addr, value),
                MemoryRegion::Palette if addr & 0x7FFF < 0x400 => {
                    HW::write_palette_ram(&mut self.gpu.engine_a, addr, value)
                }
                MemoryRegion::Palette => HW::write_palette_ram(&mut self.gpu.engine_b, addr, value),
                MemoryRegion::VRAM => self.gpu.vram.arm9_write(addr, value),
                MemoryRegion::OAM if addr & 0x7FFF < 0x400 => HW::write_mem(
                    &mut self.gpu.engine_a.oam,
                    addr & GPU::OAM_MASK as u32,
                    value,
                ),
                MemoryRegion::OAM => HW::write_mem(
                    &mut self.gpu.engine_b.oam,
                    addr & GPU::OAM_MASK as u32,
                    value,
                ),
                MemoryRegion::GBAROM => (),
                MemoryRegion::GBARAM => todo!(),
                MemoryRegion::Unknown => warn!("Writing to Unknown 0x{:08X} = 0x{:X}", addr, value),
            }
        }
    }

    fn arm9_read_io<T: MemoryValue>(&mut self, addr: u32) -> T {
        match size_of::<T>() {
            1 => num::cast::<u8, T>(self.arm9_read_io8(addr)).unwrap(),
            2 => num::cast::<u16, T>(self.arm9_read_io16(addr)).unwrap(),
            4 => num::cast::<u32, T>(self.arm9_read_io32(addr)).unwrap(),
            _ => unreachable!(),
        }
    }

    fn arm9_write_io<T: MemoryValue>(&mut self, addr: u32, value: T) {
        match size_of::<T>() {
            1 => self.arm9_write_io8(addr, num::cast::<T, u8>(value).unwrap()),
            2 => self.arm9_write_io16(addr, num::cast::<T, u16>(value).unwrap()),
            4 => self.arm9_write_io32(addr, num::cast::<T, u32>(value).unwrap()),
            _ => unreachable!(),
        }
    }

    pub fn arm9_get_access_time<T: MemoryValue>(
        &mut self,
        _access_type: AccessType,
        _addr: u32,
    ) -> usize {
        // TODO: Use accurate timings
        1
    }

    pub fn init_arm9_page_tables(&mut self) {
        Self::map_page_table(
            &mut self.arm9_page_table,
            HW::ARM9_PAGE_SHIFT,
            HW::ARM9_PAGE_SIZE,
            0x02000000,
            0x03000000,
            &mut self.main_mem,
        );
        Self::map_page_table(
            &mut self.arm9_page_table,
            HW::ARM9_PAGE_SHIFT,
            HW::ARM9_PAGE_SIZE,
            0xFFFF0000,
            0x1_0000_0000,
            &mut self.bios9,
        );

        // DTCM has second priority
        let dtcm_range = self.cp15.dtcm_range();
        Self::map_page_table(
            &mut self.arm9_page_table,
            HW::ARM9_PAGE_SHIFT,
            HW::ARM9_PAGE_SIZE,
            dtcm_range.start as usize,
            dtcm_range.end as usize,
            &mut self.dtcm,
        );
        // ITCM has highest priority
        let itcm_range = self.cp15.itcm_range();
        Self::map_page_table(
            &mut self.arm9_page_table,
            HW::ARM9_PAGE_SHIFT,
            HW::ARM9_PAGE_SIZE,
            itcm_range.start as usize,
            itcm_range.end as usize,
            &mut self.itcm,
        );
    }

    pub fn init_arm9(&mut self) -> u32 {
        let start_addr = self.cartridge.header().arm9_ram_addr;
        let rom_offset = self.cartridge.header().arm9_rom_offset as usize;
        let size = self.cartridge.header().arm9_size;
        for (i, addr) in (start_addr..start_addr + size).enumerate() {
            self.arm9_write(addr, self.cartridge.rom()[rom_offset + i]);
        }
        self.arm9_write(0x23FFC80, 0x5u8);
        self.cartridge.header().arm9_entry_addr
    }

    fn write_palette_ram<E: EngineType, T: MemoryValue>(
        engine: &mut Engine2D<E>,
        addr: u32,
        value: T,
    ) {
        let addr = addr as usize;
        match std::mem::size_of::<T>() {
            1 => (), // Ignore byte writes
            2 => engine.write_palette_ram(addr, num::cast::<T, u16>(value).unwrap()),
            4 => {
                let value = num::cast::<T, u32>(value).unwrap();
                engine.write_palette_ram(addr, value as u16);
                engine.write_palette_ram(addr + 2, (value >> 16) as u16);
            }
            _ => unreachable!(),
        }
    }
}

#[derive(PartialEq)]
pub enum ARM9MemoryRegion {
    SharedWRAM,
    IO,
    Palette,
    VRAM,
    OAM,
    GBAROM,
    GBARAM,
    Unknown,
}

impl ARM9MemoryRegion {
    pub fn from_addr(addr: u32) -> Self {
        use ARM9MemoryRegion::*;
        match addr >> 24 {
            0x3 => SharedWRAM,
            0x4 => IO,
            0x5 => Palette,
            0x6 => VRAM,
            0x7 => OAM,
            0x8 | 0x9 => GBAROM,
            0xA => GBARAM,
            _ => {
                warn!("Uknown Memory Access: {:X}", addr);
                Unknown
            }
        }
    }
}
