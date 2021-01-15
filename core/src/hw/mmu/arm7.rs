use super::{AccessType, HW, MemoryValue, IORegister};

type MemoryRegion = ARM7MemoryRegion;

impl HW {
    pub fn arm7_read<T: MemoryValue>(&mut self, addr: u32) -> T {
        match MemoryRegion::from_addr(addr) {
            MemoryRegion::BIOS => HW::read_mem(&self.bios7, addr),
            MemoryRegion::MainMem => HW::read_mem(&self.main_mem, addr & HW::MAIN_MEM_MASK),
            MemoryRegion::SharedWRAM if self.wramcnt.arm7_mask == 0 => {
                warn!("Reading from Unmapped ARM7 Shared WRAM: 0x{:X}", addr);
                HW::read_mem(&self.iwram, addr & HW::IWRAM_MASK)
            },
            MemoryRegion::SharedWRAM => HW::read_mem(&self.shared_wram,
                self.wramcnt.arm7_offset + (addr & self.wramcnt.arm7_mask)),
            MemoryRegion::IWRAM => HW::read_mem(&self.iwram, addr & HW::IWRAM_MASK),
            MemoryRegion::IO if (0x0410_0000 ..= 0x0410_0003).contains(&addr) => self.ipc_fifo_recv(false, addr),
            MemoryRegion::IO => HW::read_from_bytes(self, &HW::arm7_read_io_register, addr),
            MemoryRegion::VRAM => self.gpu.vram.arm7_read(addr),
            MemoryRegion::GBAROM => self.read_gba_rom(false, addr),
            MemoryRegion::GBARAM => todo!(),
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
            MemoryRegion::IO if (0x0400_0188 ..= 0x0400_018B).contains(&addr) =>
                self.ipc_fifo_send(true, addr, value),
            MemoryRegion::IO => HW::write_from_bytes(self, &HW::arm7_write_io_register, addr, value),
            MemoryRegion::VRAM => self.gpu.vram.arm7_write(addr, value),
            MemoryRegion::GBAROM => (),
            MemoryRegion::GBARAM => todo!(),
        }
    }

    fn arm7_read_io_register(&self, addr: u32) -> u8 {
        match addr {
            0x0400_0004 => self.gpu.dispstat7.read(0),
            0x0400_0005 => self.gpu.dispstat7.read(1),
            0x0400_0006 => (self.gpu.vcount >> 0) as u8,
            0x0400_0007 => (self.gpu.vcount >> 8) as u8,
            0x0400_00B0 ..= 0x0400_00BB => self.dma7.read(0, addr - 0xB0),
            0x0400_00BC ..= 0x0400_00C7 => self.dma7.read(1, addr - 0xBC),
            0x0400_00C8 ..= 0x0400_00D3 => self.dma7.read(2, addr - 0xC8),
            0x0400_00D4 ..= 0x0400_00DF => self.dma7.read(3, addr - 0xD4),
            0x0400_0100 ..= 0x0400_0103 => self.timers[0][0].read(&self.scheduler, addr as usize % 4),
            0x0400_0104 ..= 0x0400_0107 => self.timers[0][1].read(&self.scheduler, addr as usize % 4),
            0x0400_0108 ..= 0x0400_010B => self.timers[0][2].read(&self.scheduler, addr as usize % 4),
            0x0400_010C ..= 0x0400_010F => self.timers[0][3].read(&self.scheduler, addr as usize % 4),
            0x0400_0130 => self.keypad.keyinput.read(0),
            0x0400_0131 => self.keypad.keyinput.read(1),
            0x0400_0132 => self.keypad.keycnt.read(0),
            0x0400_0133 => self.keypad.keycnt.read(1),
            0x0400_0134 ..= 0x0400_0135 => 0, // TODO: Debug RCNT
            0x0400_0136 => self.keypad.extkeyin.read(0),
            0x0400_0137 => self.keypad.extkeyin.read(1),
            0x0400_0138 ..= 0x0400_0139 => 0, // TODO: RTC
            0x0400_0180 => self.ipc.read_sync7(0),
            0x0400_0181 => self.ipc.read_sync7(1),
            0x0400_0182 => self.ipc.read_sync7(2),
            0x0400_0183 => self.ipc.read_sync7(3),
            0x0400_0184 => self.ipc.read_fifocnt7(0),
            0x0400_0185 => self.ipc.read_fifocnt7(1),
            0x0400_0186 => self.ipc.read_fifocnt7(2),
            0x0400_0187 => self.ipc.read_fifocnt7(3),
            0x0400_01A0 => self.cartridge.spicnt.read(self.exmem.nds_arm7_access, 0),
            0x0400_01A1 => self.cartridge.spicnt.read(self.exmem.nds_arm7_access, 1),
            0x0400_01A2 => self.cartridge.read_spi_data(self.exmem.nds_arm7_access),
            0x0400_01A3 => 0, // Upper byte of AUXSPIDATA is always 0
            0x0400_01C0 => self.spi.read_cnt(0),
            0x0400_01C1 => self.spi.read_cnt(1),
            0x0400_01C2 => self.spi.read_data(),
            0x0400_01C3 => 0, // SPI bug makes upper 8 bits always 0
            0x0400_0204 => self.exmem.read_arm7(),
            0x0400_0205 => self.exmem.read_common(),
            0x0400_0208 => self.interrupts[0].master_enable.read(0),
            0x0400_0209 => self.interrupts[0].master_enable.read(1),
            0x0400_020A => self.interrupts[0].master_enable.read(2),
            0x0400_020B => self.interrupts[0].master_enable.read(3),
            0x0400_0210 => self.interrupts[0].enable.read(0),
            0x0400_0211 => self.interrupts[0].enable.read(1),
            0x0400_0212 => self.interrupts[0].enable.read(2),
            0x0400_0213 => self.interrupts[0].enable.read(3),
            0x0400_0214 => self.interrupts[0].request.read(0),
            0x0400_0215 => self.interrupts[0].request.read(1),
            0x0400_0216 => self.interrupts[0].request.read(2),
            0x0400_0217 => self.interrupts[0].request.read(3),
            0x0400_0241 => self.wramcnt.read(0),
            0x0400_0300 => self.postflg7,
            0x0400_0301 => self.haltcnt.read(0),
            0x0400_0304 => self.powcnt2.read(0),
            0x0400_0305 => self.powcnt2.read(1),
            0x0400_0306 => self.powcnt2.read(2),
            0x0400_0307 => self.powcnt2.read(3),
            0x0400_0400 ..= 0x0400_051F => self.spu.read(addr as usize & 0xFFF),
            0x0480_4000 ..= 0x0480_5FFF => 0, // TODO: WiFi RAM
            0x0480_8000 ..= 0x0480_8FFF => 0, // TOOD: WiFi Registers
            _ => { warn!("Ignoring ARM7 IO Register Read at 0x{:08X}", addr); 0 }
        }
    }

    fn arm7_write_io_register(&mut self, addr: u32, value: u8) {
        match addr {
            0x0400_0004 => self.gpu.dispstat7.write(&mut self.scheduler, 0, value),
            0x0400_0005 => self.gpu.dispstat7.write(&mut self.scheduler, 1, value),
            0x0400_0006 => (), // VCOUNT is read only
            0x0400_0007 => (), // VCOUNT is read only
            0x0400_00B0 ..= 0x0400_00BB => self.dma7.write(0, &mut self.scheduler, addr - 0xB0, value),
            0x0400_00BC ..= 0x0400_00C7 => self.dma7.write(1, &mut self.scheduler, addr - 0xBC, value),
            0x0400_00C8 ..= 0x0400_00D3 => self.dma7.write(2, &mut self.scheduler, addr - 0xC8, value),
            0x0400_00D4 ..= 0x0400_00DF => self.dma7.write(3, &mut self.scheduler, addr - 0xD4, value),
            0x0400_0100 ..= 0x0400_0103 => self.timers[0][0].write(&mut self.scheduler, addr as usize % 4, value),
            0x0400_0104 ..= 0x0400_0107 => self.timers[0][1].write(&mut self.scheduler, addr as usize % 4, value),
            0x0400_0108 ..= 0x0400_010B => self.timers[0][2].write(&mut self.scheduler, addr as usize % 4, value),
            0x0400_010C ..= 0x0400_010F => self.timers[0][3].write(&mut self.scheduler, addr as usize % 4, value),
            0x0400_0134 ..= 0x0400_0135 => (), // TODO: Debug RCNT
            0x0400_0136 => self.keypad.extkeyin.write(&mut self.scheduler, 0, value),
            0x0400_0137 => self.keypad.extkeyin.write(&mut self.scheduler, 1, value),
            0x0400_0138 ..= 0x0400_0139 => (), // TODO: RTC
            0x0400_0180 => self.interrupts[1].request |= self.ipc.write_sync7(0, value),
            0x0400_0181 => self.interrupts[1].request |= self.ipc.write_sync7(1, value),
            0x0400_0182 => self.interrupts[1].request |= self.ipc.write_sync7(2, value),
            0x0400_0183 => self.interrupts[1].request |= self.ipc.write_sync7(3, value),
            0x0400_0184 => self.interrupts[0].request |= self.ipc.write_fifocnt7(0, value),
            0x0400_0185 => self.interrupts[0].request |= self.ipc.write_fifocnt7(1, value),
            0x0400_0186 => self.interrupts[0].request |= self.ipc.write_fifocnt7(2, value),
            0x0400_0187 => self.interrupts[0].request |= self.ipc.write_fifocnt7(3, value),
            0x0400_01A0 => self.cartridge.spicnt.write(self.exmem.nds_arm7_access, 0, value),
            0x0400_01A1 => self.cartridge.spicnt.write(self.exmem.nds_arm7_access, 1, value),
            0x0400_01A2 => self.cartridge.write_spi_data(self.exmem.nds_arm7_access, value),
            0x0400_01A3 => (), // TODO: Does this write do anything?
            0x0400_01C0 => self.spi.write_cnt(&mut self.scheduler, 0, value),
            0x0400_01C1 => self.spi.write_cnt(&mut self.scheduler, 1, value),
            0x0400_01C2 => self.spi.write_data(value),
            0x0400_01C3 => (), // SPI bug makes upper 8 bits always 0
            0x0400_0204 => self.exmem.write_arm7(value),
            0x0400_0205 => (), // Upper bits are read-only for ARM7
            0x0400_0208 => self.interrupts[0].master_enable.write(&mut self.scheduler, 0, value),
            0x0400_0209 => self.interrupts[0].master_enable.write(&mut self.scheduler, 1, value),
            0x0400_020A => self.interrupts[0].master_enable.write(&mut self.scheduler, 2, value),
            0x0400_020B => self.interrupts[0].master_enable.write(&mut self.scheduler, 3, value),
            0x0400_0210 => self.interrupts[0].enable.write(&mut self.scheduler, 0, value),
            0x0400_0211 => self.interrupts[0].enable.write(&mut self.scheduler, 1, value),
            0x0400_0212 => self.interrupts[0].enable.write(&mut self.scheduler, 2, value),
            0x0400_0213 => self.interrupts[0].enable.write(&mut self.scheduler, 3, value),
            0x0400_0214 => self.interrupts[0].request.write(&mut self.scheduler, 0, value),
            0x0400_0215 => self.interrupts[0].request.write(&mut self.scheduler, 1, value),
            0x0400_0216 => self.interrupts[0].request.write(&mut self.scheduler, 2, value),
            0x0400_0217 => self.interrupts[0].request.write(&mut self.scheduler, 3, value),
            0x0400_0241 => (), // WRAMCNT is read-only
            0x0400_0300 => (), // POSTFLG
            0x0400_0301 => self.haltcnt.write(&mut self.scheduler, 0, value),
            0x0400_0304 => self.powcnt2.write(&mut self.scheduler, 0, value),
            0x0400_0305 => self.powcnt2.write(&mut self.scheduler, 1, value),
            0x0400_0306 => self.powcnt2.write(&mut self.scheduler, 2, value),
            0x0400_0307 => self.powcnt2.write(&mut self.scheduler, 3, value),
            0x0400_0400 ..= 0x0400_051F => self.spu.write(&mut self.scheduler, addr as usize & 0xFFF, value),
            0x0480_4000 ..= 0x0480_5FFF => (), // TODO: WiFi RAM
            0x0480_8000 ..= 0x0480_8FFF => (), // TOOD: WiFi Registers
            _ => warn!("Ignoring ARM7 IO Register Write 0x{:08X} = {:02X}", addr, value),
        }
    }

    pub fn arm7_get_access_time<T: MemoryValue>(&mut self, _access_type: AccessType, _addr: u32) -> usize {
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
