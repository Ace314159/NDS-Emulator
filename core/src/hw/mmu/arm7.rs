use super::{AccessType, HW, MemoryValue, IORegister};

type MemoryRegion = ARM7MemoryRegion;

impl HW {
    pub fn arm7_read<T: MemoryValue>(&self, addr: u32) -> T {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::BIOS => HW::read_mem(&self.bios7, addr),
            MemoryRegion::MainMem => HW::read_mem(&self.main_mem, addr & HW::MAIN_MEM_MASK),
            MemoryRegion::SharedWRAM if self.wramcnt.arm7_mask == 0 =>
                HW::read_mem(&self.iwram, addr & HW::IWRAM_MASK),
            MemoryRegion::SharedWRAM => HW::read_mem(&self.shared_wram,
                self.wramcnt.arm7_offset + addr & self.wramcnt.arm7_mask),
            MemoryRegion::IWRAM => HW::read_mem(&self.iwram, addr & HW::IWRAM_MASK),
            MemoryRegion::IO => HW::read_from_bytes(self, &HW::arm7_read_io_register, addr),
        }
    }

    pub fn arm7_write<T: MemoryValue>(&mut self, addr: u32, value: T) {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::BIOS => warn!("Writing to BIOS7 0x{:08x} = 0x{:X}", addr, value),
            MemoryRegion::MainMem => HW::write_mem(&mut self.main_mem, addr & HW::MAIN_MEM_MASK, value),
            MemoryRegion::SharedWRAM if self.wramcnt.arm7_mask == 0 =>
                HW::write_mem(&mut self.iwram, addr & HW::IWRAM_MASK, value),
            MemoryRegion::SharedWRAM => HW::write_mem(&mut self.shared_wram,
                self.wramcnt.arm7_offset + addr & self.wramcnt.arm7_mask, value),
            MemoryRegion::IWRAM => HW::write_mem(&mut self.iwram, addr & HW::IWRAM_MASK, value),
            MemoryRegion::IO => HW::write_from_bytes(self, &HW::arm7_write_io_register, addr, value),
        }
    }

    fn arm7_read_io_register(&self, addr: u32) -> u8 {
        match addr {
            0x0400_0130 => self.keypad.keyinput.read(0),
            0x0400_0131 => self.keypad.keyinput.read(1),
            0x0400_0132 => self.keypad.keycnt.read(0),
            0x0400_0133 => self.keypad.keycnt.read(1),
            0x0400_0136 => self.keypad.extkeyin.read(0),
            0x0400_0137 => self.keypad.extkeyin.read(1),
            0x0400_0200 => self.interrupts7.enable.read(0),
            0x0400_0201 => self.interrupts7.enable.read(1),
            0x0400_0202 => self.interrupts7.request.read(0),
            0x0400_0203 => self.interrupts7.request.read(1),
            0x0400_0208 => self.interrupts7.master_enable.read(0),
            0x0400_0209 => self.interrupts7.master_enable.read(1),
            _ => { warn!("Ignoring ARM7 IO Register Read at 0x{:08X}", addr); 0 }
        }
    }

    fn arm7_write_io_register(&mut self, addr: u32, value: u8) {
        match addr {
            0x0400_0136 => self.keypad.extkeyin.write(&mut self.scheduler, 0, value),
            0x0400_0137 => self.keypad.extkeyin.write(&mut self.scheduler, 1, value),
            0x0400_0200 => self.interrupts7.enable.write(&mut self.scheduler, 0, value),
            0x0400_0201 => self.interrupts7.enable.write(&mut self.scheduler, 1, value),
            0x0400_0202 => self.interrupts7.request.write(&mut self.scheduler, 0, value),
            0x0400_0203 => self.interrupts7.request.write(&mut self.scheduler, 1, value),
            0x0400_0208 => self.interrupts7.master_enable.write(&mut self.scheduler, 0, value),
            0x0400_0209 => self.interrupts7.master_enable.write(&mut self.scheduler, 1, value),
            _ => warn!("Ignoring ARM7 IO Register Write 0x{:08X} = {:02X}", addr, value),
        }
    }

    pub fn arm7_get_access_time<T: MemoryValue>(&mut self, _access_type: AccessType, _addr: u32) -> usize {
        // TODO: Use accurate timings
        1
    }

    pub fn init_arm7(&mut self) -> u32 {
        let start_addr = self.rom_header.arm7_ram_addr;
        let rom_offset = self.rom_header.arm7_rom_offset as usize;
        let size = self.rom_header.arm7_size;
        for (i, addr) in (start_addr..start_addr + size).enumerate() {
            self.arm7_write(addr, self.rom[rom_offset + i]);
        }
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
