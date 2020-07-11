use super::{AccessType, HW, MemoryValue, IORegister};

type MemoryRegion = ARM7MemoryRegion;

impl HW {
    pub fn arm7_read<T: MemoryValue>(&self, addr: u32) -> T {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::BIOS => HW::read_mem(&self.bios7, addr),
            MemoryRegion::MainMem => HW::read_mem(&self.main_mem, addr & HW::MAIN_MEM_MASK),
            MemoryRegion::SharedWRAM => todo!(),
            MemoryRegion::IWRAM => HW::read_mem(&self.iwram, addr & HW::IWRAM_MASK),
            MemoryRegion::IO => HW::read_from_bytes(self, &HW::arm7_read_io_register, addr),
        }
    }

    pub fn arm7_write<T: MemoryValue>(&mut self, addr: u32, value: T) {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::BIOS => warn!("Writing to BIOS7 0x{:08x} = 0x{:X}", addr, value),
            MemoryRegion::MainMem => HW::write_mem(&mut self.main_mem, addr & HW::MAIN_MEM_MASK, value),
            MemoryRegion::SharedWRAM => todo!(),
            MemoryRegion::IWRAM => HW::write_mem(&mut self.iwram, addr & HW::IWRAM_MASK, value),
            MemoryRegion::IO => HW::write_from_bytes(self, &HW::arm7_write_io_register, addr, value),
        }
    }

    fn arm7_read_io_register(&self, addr: u32) -> u8 {
        match addr {
            0x04000130 => self.keypad.keyinput.read(0),
            0x04000131 => self.keypad.keyinput.read(1),
            0x04000132 => self.keypad.keycnt.read(0),
            0x04000133 => self.keypad.keycnt.read(1),
            0x04000136 => self.keypad.extkeyin.read(0),
            0x04000137 => self.keypad.extkeyin.read(1),
            0x04000200 => self.interrupts7.enable.read(0),
            0x04000201 => self.interrupts7.enable.read(1),
            0x04000202 => self.interrupts7.request.read(0),
            0x04000203 => self.interrupts7.request.read(1),
            0x04000208 => self.interrupts7.master_enable.read(0),
            0x04000209 => self.interrupts7.master_enable.read(1),
            _ => { warn!("Ignoring ARM7 IO Register Read at 0x{:08X}", addr); 0 }
        }
    }

    fn arm7_write_io_register(&mut self, addr: u32, value: u8) {
        match addr {
            0x04000136 => self.keypad.extkeyin.write(&mut self.scheduler, 0, value),
            0x04000137 => self.keypad.extkeyin.write(&mut self.scheduler, 1, value),
            0x04000200 => self.interrupts7.enable.write(&mut self.scheduler, 0, value),
            0x04000201 => self.interrupts7.enable.write(&mut self.scheduler, 1, value),
            0x04000202 => self.interrupts7.request.write(&mut self.scheduler, 0, value),
            0x04000203 => self.interrupts7.request.write(&mut self.scheduler, 1, value),
            0x04000208 => self.interrupts7.master_enable.write(&mut self.scheduler, 0, value),
            0x04000209 => self.interrupts7.master_enable.write(&mut self.scheduler, 1, value),
            _ => warn!("Ignoring ARM7 IO Register Write 0x{:08X} = {:02X}", addr, value),
        }
    }

    pub fn arm7_get_access_time<T: MemoryValue>(&mut self, _access_type: AccessType, _addr: u32) -> usize {
        // TODO: Use accurate timings
        1
    }

    pub fn init_arm7(&mut self) -> u32 {
        let region = MemoryRegion::from_addr(self.rom_header.arm7_ram_addr);
        let addr = self.rom_header.arm7_ram_addr as usize;
        let rom_offset = self.rom_header.arm7_rom_offset as usize;
        let size = self.rom_header.arm7_size as usize;
        match region {
            MemoryRegion::MainMem => {
                let addr = addr & HW::MAIN_MEM_MASK as usize;
                self.main_mem[addr..addr + size].copy_from_slice(&self.rom[rom_offset..rom_offset + size])
            },
            MemoryRegion::IWRAM => {
                let addr = addr & HW::IWRAM_MASK as usize;
                self.iwram[addr..addr + size].copy_from_slice(&self.rom[rom_offset..rom_offset + size])
            },
            _ => panic!("Invalid ARM7 Entry Address: 0x{:08X}", self.rom_header.arm7_ram_addr),
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
