mod io;

use super::{AccessType, IORegister, MemoryValue, CP15, HW};
use crate::hw::gpu::{Engine2D, EngineType, GPU};
use crate::num;
use std::mem::size_of;

type MemoryRegion = ARM9MemoryRegion;

impl HW {
    const ITCM_MASK: u32 = HW::ITCM_SIZE as u32 - 1;
    const DTCM_MASK: u32 = HW::DTCM_SIZE as u32 - 1;

    pub fn arm9_read<T: MemoryValue>(&mut self, addr: u32) -> T {
        match MemoryRegion::from_addr(addr, &self.cp15) {
            MemoryRegion::ITCM => HW::read_mem(&self.itcm, addr & HW::ITCM_MASK),
            MemoryRegion::DTCM => HW::read_mem(&self.dtcm, addr & HW::DTCM_MASK),
            MemoryRegion::MainMem => HW::read_mem(&self.main_mem, addr & HW::MAIN_MEM_MASK),
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
            MemoryRegion::BIOS => HW::read_mem(&self.bios9, addr & 0xFFFF),
            MemoryRegion::Unknown => {
                warn!("Reading from Unknown 0x{:08X}", addr);
                num::zero()
            }
        }
    }

    pub fn arm9_write<T: MemoryValue>(&mut self, addr: u32, value: T) {
        match MemoryRegion::from_addr(addr, &self.cp15) {
            MemoryRegion::ITCM => HW::write_mem(&mut self.itcm, addr & HW::ITCM_MASK, value),
            MemoryRegion::DTCM => HW::write_mem(&mut self.dtcm, addr & HW::DTCM_MASK, value),
            MemoryRegion::MainMem => {
                HW::write_mem(&mut self.main_mem, addr & HW::MAIN_MEM_MASK, value)
            }
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
            MemoryRegion::BIOS => warn!("Writing to BIOS9 0x{:08x} = 0x{:X}", addr, value),
            MemoryRegion::Unknown => warn!("Writing to Unknown 0x{:08X} = 0x{:X}", addr, value),
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
    ITCM,
    DTCM,
    MainMem,
    SharedWRAM,
    IO,
    Palette,
    VRAM,
    OAM,
    GBAROM,
    GBARAM,
    BIOS,
    Unknown,
}

impl ARM9MemoryRegion {
    pub fn from_addr(addr: u32, cp15: &CP15) -> Self {
        use ARM9MemoryRegion::*;
        if cp15.addr_in_itcm(addr) {
            return ITCM;
        }
        if cp15.addr_in_dtcm(addr) {
            return DTCM;
        }
        match addr >> 24 {
            0x2 => MainMem,
            0x3 => SharedWRAM,
            0x4 => IO,
            0x5 => Palette,
            0x6 => VRAM,
            0x7 => OAM,
            0x8 | 0x9 => GBAROM,
            0xA => GBARAM,
            0xFF if addr >> 16 == 0xFFFF => BIOS,
            _ => {
                warn!("Uknown Memory Access: {:X}", addr);
                Unknown
            }
        }
    }
}
