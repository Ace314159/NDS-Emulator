use crate::num;
use super::{AccessType, HW, MemoryValue, IORegister};
use crate::hw::gpu::Engine2D;

type MemoryRegion = ARM9MemoryRegion;

impl HW {
    pub fn arm9_read<T: MemoryValue>(&self, addr: u32) -> T {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::MainMem => HW::read_mem(&self.main_mem, addr & HW::MAIN_MEM_MASK),
            MemoryRegion::WRAM => todo!(),
            MemoryRegion::SharedWRAM => todo!(),
            MemoryRegion::IO => HW::read_from_bytes(self, &HW::arm9_read_io_register, addr),
            MemoryRegion::Palette =>
                HW::read_from_bytes(self.gpu_engine(addr),&Engine2D::read_palette_ram, addr),
            MemoryRegion::VRAM => self.read_vram(addr),
            MemoryRegion::OAM => HW::read_mem(&self.gpu_engine(addr).oam, addr),
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
            MemoryRegion::IO => HW::write_from_bytes(self, &HW::arm9_write_io_register, addr, value),
            MemoryRegion::Palette =>
                HW::write_from_bytes(self.gpu_engine_mut(addr),&Engine2D::write_palette_ram, addr, value),
            MemoryRegion::VRAM => self.write_vram(addr, value),
            MemoryRegion::OAM => HW::write_mem(&mut self.gpu_engine_mut(addr).oam, addr, value),
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
        let region = MemoryRegion::from_addr(self.rom_header.arm9_ram_addr);
        let addr = self.rom_header.arm9_ram_addr as usize & 0xFF_FFFF;
        let rom_offset = self.rom_header.arm9_rom_offset as usize;
        let size = self.rom_header.arm9_size as usize;
        match region {
            MemoryRegion::MainMem =>
                self.main_mem[addr..addr + size].copy_from_slice(&self.rom[rom_offset..rom_offset + size]),
            _ => panic!("Invalid ARM9 Entry Address: 0x{:08X}", self.rom_header.arm9_ram_addr),
        };
        self.rom_header.arm9_entry_addr
    }

    fn arm9_read_io_register(&self, addr: u32) -> u8 {
        match addr {
            0x0400_0000 ..= 0x0400_006F => self.gpu.engine_a.read_register(addr),
            0x0400_1000 ..= 0x0400_106F => self.gpu.engine_b.read_register(addr),
            0x04000200 => self.interrupts9.enable.read(0),
            0x04000201 => self.interrupts9.enable.read(1),
            0x04000202 => self.interrupts9.request.read(0),
            0x04000203 => self.interrupts9.request.read(1),
            0x04000208 => self.interrupts9.master_enable.read(0),
            0x04000209 => self.interrupts9.master_enable.read(1),
            0x0400_0240 ..= 0x0400_0246 => 0,
            0x0400_0248 ..= 0x0400_0249 => 0,
            _ => { warn!("Ignoring ARM9 IO Register Read at 0x{:08X}", addr); 0 }
        }
    }

    fn arm9_write_io_register(&mut self, addr: u32, value: u8) {
        match addr {
            0x0400_0000 ..= 0x0400_006F => self.gpu.engine_a.write_register(&mut self.scheduler, addr, value),
            0x04000200 => self.interrupts9.enable.write(&mut self.scheduler, 0, value),
            0x04000201 => self.interrupts9.enable.write(&mut self.scheduler, 1, value),
            0x04000202 => self.interrupts9.request.write(&mut self.scheduler, 0, value),
            0x04000203 => self.interrupts9.request.write(&mut self.scheduler, 1, value),
            0x04000208 => self.interrupts9.master_enable.write(&mut self.scheduler, 0, value),
            0x04000209 => self.interrupts9.master_enable.write(&mut self.scheduler, 1, value),
            0x0400_1000 ..= 0x0400_106F => self.gpu.engine_b.write_register(&mut self.scheduler, addr, value),
            0x0400_0240 ..= 0x0400_0246 => self.gpu.vram.write_vram_cnt(addr as usize & 0xF, value),
            0x0400_0248 ..= 0x0400_0249 => self.gpu.vram.write_vram_cnt(addr as usize & 0xF - 1, value),
            _ => warn!("Ignoring ARM9 IO Register Write 0x{:08X} = {:02X}", addr, value),
        }
    }

    fn gpu_engine(&self, addr: u32) -> &Engine2D {
        if addr & 0xFFF < 0x400 { &self.gpu.engine_a } else { &self.gpu.engine_b }
    }

    fn gpu_engine_mut(&mut self, addr: u32) -> &mut Engine2D {
        if addr & 0xFFF < 0x400 { &mut self.gpu.engine_a } else { &mut self.gpu.engine_b }
    }

    fn read_vram<T: MemoryValue>(&self, addr: u32) -> T {
        if let Some((mem, addr)) = self.gpu.vram.get_mem(addr) {
            HW::read_mem(mem, addr)
        } else { num::zero() }
    }

    fn write_vram<T: MemoryValue>(&mut self, addr: u32, value: T) {
        if let Some((mem, addr)) = self.gpu.vram.get_mem_mut(addr) {
            HW::write_mem(mem, addr, value)
        }
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
