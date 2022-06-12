use super::{IORegister, HW};

impl HW {
    pub(super) fn arm9_read_io8(&self, addr: u32) -> u8 {
        match addr {
            0x0400_0000..=0x0400_0003 => self.gpu.engine_a.read_register(addr),
            0x0400_0004 => self.gpu.dispstats[1].read(0),
            0x0400_0005 => self.gpu.dispstats[1].read(1),
            0x0400_0006 => (self.gpu.vcount >> 0) as u8,
            0x0400_0007 => (self.gpu.vcount >> 8) as u8,
            0x0400_0008..=0x0400_005F => self.gpu.engine_a.read_register(addr),
            0x0400_0060..=0x0400_0063 => self.gpu.engine3d.disp3dcnt.read(addr as usize % 4),
            0x0400_0064..=0x0400_0067 => self.gpu.dispcapcnt.read(addr as usize % 4),
            0x0400_006C => self.gpu.engine_a.master_bright.read(0),
            0x0400_006D => self.gpu.engine_a.master_bright.read(1),
            0x0400_006E => self.gpu.engine_a.master_bright.read(2),
            0x0400_006F => self.gpu.engine_a.master_bright.read(3),
            0x0400_00B0..=0x0400_00BB => self.dmas[1].read(0, addr - 0xB0),
            0x0400_00BC..=0x0400_00C7 => self.dmas[1].read(1, addr - 0xBC),
            0x0400_00C8..=0x0400_00D3 => self.dmas[1].read(2, addr - 0xC8),
            0x0400_00D4..=0x0400_00DF => self.dmas[1].read(3, addr - 0xD4),
            0x0400_00E0..=0x0400_00E3 => {
                HW::read_byte_from_value(&self.dma_fill[0], addr as usize % 4)
            }
            0x0400_00E4..=0x0400_00E7 => {
                HW::read_byte_from_value(&self.dma_fill[1], addr as usize % 4)
            }
            0x0400_00E8..=0x0400_00EB => {
                HW::read_byte_from_value(&self.dma_fill[2], addr as usize % 4)
            }
            0x0400_00EC..=0x0400_00EF => {
                HW::read_byte_from_value(&self.dma_fill[3], addr as usize % 4)
            }
            0x0400_0100..=0x0400_0103 => self.timers[1][0].read(&self.scheduler, addr as usize % 4),
            0x0400_0104..=0x0400_0107 => self.timers[1][1].read(&self.scheduler, addr as usize % 4),
            0x0400_0108..=0x0400_010B => self.timers[1][2].read(&self.scheduler, addr as usize % 4),
            0x0400_010C..=0x0400_010F => self.timers[1][3].read(&self.scheduler, addr as usize % 4),
            0x0400_0130 => self.keypad.keyinput.read(0),
            0x0400_0131 => self.keypad.keyinput.read(1),
            0x0400_0132 => self.keypad.keycnt.read(0),
            0x0400_0133 => self.keypad.keycnt.read(1),
            0x0400_0136 => self.keypad.extkeyin.read(0),
            0x0400_0137 => self.keypad.extkeyin.read(1),
            0x0400_0180 => self.ipc.read_sync9(0),
            0x0400_0181 => self.ipc.read_sync9(1),
            0x0400_0182 => self.ipc.read_sync9(2),
            0x0400_0183 => self.ipc.read_sync9(3),
            0x0400_0184 => self.ipc.read_fifocnt9(0),
            0x0400_0185 => self.ipc.read_fifocnt9(1),
            0x0400_0186 => self.ipc.read_fifocnt9(2),
            0x0400_0187 => self.ipc.read_fifocnt9(3),
            0x0400_01A0 => self.cartridge.spicnt.read(!self.exmem.nds_arm7_access, 0),
            0x0400_01A1 => self.cartridge.spicnt.read(!self.exmem.nds_arm7_access, 1),
            0x0400_01A2 => self.cartridge.read_spi_data(!self.exmem.nds_arm7_access),
            0x0400_01A3 => 0, // Upper byte of AUXSPIDATA is always 0
            0x0400_01A4 => self.cartridge.read_romctrl(!self.exmem.nds_arm7_access, 0),
            0x0400_01A5 => self.cartridge.read_romctrl(!self.exmem.nds_arm7_access, 1),
            0x0400_01A6 => self.cartridge.read_romctrl(!self.exmem.nds_arm7_access, 2),
            0x0400_01A7 => self.cartridge.read_romctrl(!self.exmem.nds_arm7_access, 3),
            0x0400_0204 => self.exmem.read_arm9(),
            0x0400_0205 => self.exmem.read_common(),
            0x0400_0208 => self.interrupts[1].master_enable.read(0),
            0x0400_0209 => self.interrupts[1].master_enable.read(1),
            0x0400_020A => self.interrupts[1].master_enable.read(2),
            0x0400_020B => self.interrupts[1].master_enable.read(3),
            0x0400_0210 => self.interrupts[1].enable.read(0),
            0x0400_0211 => self.interrupts[1].enable.read(1),
            0x0400_0212 => self.interrupts[1].enable.read(2),
            0x0400_0213 => self.interrupts[1].enable.read(3),
            0x0400_0214 => self.interrupts[1].request.read(0),
            0x0400_0215 => self.interrupts[1].request.read(1),
            0x0400_0216 => self.interrupts[1].request.read(2),
            0x0400_0217 => self.interrupts[1].request.read(3),
            0x0400_0240..=0x0400_0246 => self.gpu.vram.read_vram_cnt(addr as usize & 0xF),
            0x0400_0247 => self.wramcnt.read(0),
            0x0400_0248..=0x0400_0249 => self.gpu.vram.read_vram_cnt((addr as usize & 0xF) - 1),
            0x0400_0280..=0x0400_0283 => self.div.cnt.read(addr as usize & 0xF),
            0x0400_0290..=0x0400_0297 => self.div.read_numer(addr as usize & 0x7),
            0x0400_0298..=0x0400_029F => self.div.read_denom(addr as usize & 0x7),
            0x0400_02A0..=0x0400_02A7 => self.div.read_quot(addr as usize & 0x7),
            0x0400_02A8..=0x0400_02AF => self.div.read_rem(addr as usize & 0x7),
            0x0400_02B0..=0x0400_02B3 => self.sqrt.cnt.read(addr as usize & 0xF),
            0x0400_02B4..=0x0400_02B7 => self.sqrt.read_result(addr as usize & 0x3),
            0x0400_02B8..=0x0400_02BF => self.sqrt.read_param(addr as usize & 0x7),
            0x0400_0300 => self.postflg9,
            0x0400_0301..=0x0400_0303 => 0, // Other Parts of POSTFLG
            0x0400_0304 => self.gpu.powcnt1.read(0),
            0x0400_0305 => self.gpu.powcnt1.read(1),
            0x0400_0306 => self.gpu.powcnt1.read(2),
            0x0400_0307 => self.gpu.powcnt1.read(3),
            0x0400_0320..=0x0400_06A3 => self.gpu.engine3d.read_register(addr),
            0x0400_1000..=0x0400_1003 => self.gpu.engine_b.read_register(addr),
            0x0400_1004..=0x0400_1007 => 0,
            0x0400_1008..=0x0400_105F => self.gpu.engine_b.read_register(addr),
            0x0400_1060..=0x0400_106B => 0,
            0x0400_106C => self.gpu.engine_b.master_bright.read(0),
            0x0400_106D => self.gpu.engine_b.master_bright.read(1),
            0x0400_106E => self.gpu.engine_b.master_bright.read(2),
            0x0400_106F => self.gpu.engine_b.master_bright.read(3),
            0x0400_4010..=0x0400_4011 => 0, // DSi register that's unused for NDS
            _ => {
                warn!("Ignoring ARM9 IO Register Read at 0x{:08X}", addr);
                0
            }
        }
    }

    pub(super) fn arm9_read_io16(&self, addr: u32) -> u16 {
        (self.arm9_read_io8(addr) as u16) << 0 | (self.arm9_read_io8(addr + 1) as u16) << 8
    }

    pub(super) fn arm9_read_io32(&mut self, addr: u32) -> u32 {
        match addr {
            0x0410_0000 => self.ipc_fifo_recv(true),
            0x0410_0010 => self.read_game_card(true),
            _ => {
                (self.arm9_read_io8(addr) as u32) << 0
                    | (self.arm9_read_io8(addr + 1) as u32) << 8
                    | (self.arm9_read_io8(addr + 2) as u32) << 16
                    | (self.arm9_read_io8(addr + 3) as u32) << 24
            }
        }
    }

    pub(super) fn arm9_write_io8(&mut self, addr: u32, value: u8) {
        match addr {
            0x0400_0000..=0x0400_0003 => {
                self.gpu
                    .engine_a
                    .write_register(&mut self.scheduler, addr, value)
            }
            0x0400_0004 => self.gpu.dispstats[1].write(&mut self.scheduler, 0, value),
            0x0400_0005 => self.gpu.dispstats[1].write(&mut self.scheduler, 1, value),
            0x0400_0006 => (), // VCOUNT is read only
            0x0400_0007 => (), // VCOUNT is read only
            0x0400_0008..=0x0400_005F => {
                self.gpu
                    .engine_a
                    .write_register(&mut self.scheduler, addr, value)
            }
            0x0400_0060..=0x0400_0063 => {
                self.gpu
                    .engine3d
                    .disp3dcnt
                    .write(&mut self.scheduler, addr as usize % 4, value)
            }
            0x0400_0064..=0x0400_0067 => {
                self.gpu
                    .dispcapcnt
                    .write(&mut self.scheduler, addr as usize % 4, value)
            }
            0x0400_006C => self
                .gpu
                .engine_a
                .master_bright
                .write(&mut self.scheduler, 0, value),
            0x0400_006D => self
                .gpu
                .engine_a
                .master_bright
                .write(&mut self.scheduler, 1, value),
            0x0400_006E => self
                .gpu
                .engine_a
                .master_bright
                .write(&mut self.scheduler, 2, value),
            0x0400_006F => self
                .gpu
                .engine_a
                .master_bright
                .write(&mut self.scheduler, 3, value),
            0x0400_00B0..=0x0400_00BB => {
                self.dmas[1].write(0, &mut self.scheduler, addr - 0xB0, value)
            }
            0x0400_00BC..=0x0400_00C7 => {
                self.dmas[1].write(1, &mut self.scheduler, addr - 0xBC, value)
            }
            0x0400_00C8..=0x0400_00D3 => {
                self.dmas[1].write(2, &mut self.scheduler, addr - 0xC8, value)
            }
            0x0400_00D4..=0x0400_00DF => {
                self.dmas[1].write(3, &mut self.scheduler, addr - 0xD4, value)
            }
            0x0400_00E0..=0x0400_00E3 => {
                HW::write_byte_to_value(&mut self.dma_fill[0], addr as usize % 4, value)
            }
            0x0400_00E4..=0x0400_00E7 => {
                HW::write_byte_to_value(&mut self.dma_fill[1], addr as usize % 4, value)
            }
            0x0400_00E8..=0x0400_00EB => {
                HW::write_byte_to_value(&mut self.dma_fill[2], addr as usize % 4, value)
            }
            0x0400_00EC..=0x0400_00EF => {
                HW::write_byte_to_value(&mut self.dma_fill[3], addr as usize % 4, value)
            }
            0x0400_0100..=0x0400_0103 => {
                self.timers[1][0].write(&mut self.scheduler, addr as usize % 4, value)
            }
            0x0400_0104..=0x0400_0107 => {
                self.timers[1][1].write(&mut self.scheduler, addr as usize % 4, value)
            }
            0x0400_0108..=0x0400_010B => {
                self.timers[1][2].write(&mut self.scheduler, addr as usize % 4, value)
            }
            0x0400_010C..=0x0400_010F => {
                self.timers[1][3].write(&mut self.scheduler, addr as usize % 4, value)
            }
            0x0400_0130 => self.keypad.keyinput.write(&mut self.scheduler, 0, value),
            0x0400_0131 => self.keypad.keyinput.write(&mut self.scheduler, 1, value),
            0x0400_0132 => self.keypad.keycnt.write(&mut self.scheduler, 0, value),
            0x0400_0133 => self.keypad.keycnt.write(&mut self.scheduler, 1, value),
            0x0400_0136 => self.keypad.extkeyin.write(&mut self.scheduler, 0, value),
            0x0400_0137 => self.keypad.extkeyin.write(&mut self.scheduler, 1, value),
            0x0400_0180 => self.interrupts[0].request |= self.ipc.write_sync9(0, value),
            0x0400_0181 => self.interrupts[0].request |= self.ipc.write_sync9(1, value),
            0x0400_0182 => self.interrupts[0].request |= self.ipc.write_sync9(2, value),
            0x0400_0183 => self.interrupts[0].request |= self.ipc.write_sync9(3, value),
            0x0400_0184 => self.interrupts[1].request |= self.ipc.write_fifocnt9(0, value),
            0x0400_0185 => self.interrupts[1].request |= self.ipc.write_fifocnt9(1, value),
            0x0400_0186 => self.interrupts[1].request |= self.ipc.write_fifocnt9(2, value),
            0x0400_0187 => self.interrupts[1].request |= self.ipc.write_fifocnt9(3, value),
            0x0400_01A0 => self
                .cartridge
                .spicnt
                .write(!self.exmem.nds_arm7_access, 0, value),
            0x0400_01A1 => self
                .cartridge
                .spicnt
                .write(!self.exmem.nds_arm7_access, 1, value),
            0x0400_01A2 => self
                .cartridge
                .write_spi_data(!self.exmem.nds_arm7_access, value),
            0x0400_01A3 => (), // TODO: Does this write do anything?
            0x0400_01A4 => self.cartridge.write_romctrl(
                &mut self.scheduler,
                true,
                !self.exmem.nds_arm7_access,
                0,
                value,
            ),
            0x0400_01A5 => self.cartridge.write_romctrl(
                &mut self.scheduler,
                true,
                !self.exmem.nds_arm7_access,
                1,
                value,
            ),
            0x0400_01A6 => self.cartridge.write_romctrl(
                &mut self.scheduler,
                true,
                !self.exmem.nds_arm7_access,
                2,
                value,
            ),
            0x0400_01A7 => self.cartridge.write_romctrl(
                &mut self.scheduler,
                true,
                !self.exmem.nds_arm7_access,
                3,
                value,
            ),
            0x0400_01A8 => self
                .cartridge
                .write_command(!self.exmem.nds_arm7_access, 0, value),
            0x0400_01A9 => self
                .cartridge
                .write_command(!self.exmem.nds_arm7_access, 1, value),
            0x0400_01AA => self
                .cartridge
                .write_command(!self.exmem.nds_arm7_access, 2, value),
            0x0400_01AB => self
                .cartridge
                .write_command(!self.exmem.nds_arm7_access, 3, value),
            0x0400_01AC => self
                .cartridge
                .write_command(!self.exmem.nds_arm7_access, 4, value),
            0x0400_01AD => self
                .cartridge
                .write_command(!self.exmem.nds_arm7_access, 5, value),
            0x0400_01AE => self
                .cartridge
                .write_command(!self.exmem.nds_arm7_access, 6, value),
            0x0400_01AF => self
                .cartridge
                .write_command(!self.exmem.nds_arm7_access, 7, value),
            0x0400_0204 => self.exmem.write_arm9(value),
            0x0400_0205 => self.exmem.write_common(value),
            0x0400_0208 => self.interrupts[1]
                .master_enable
                .write(&mut self.scheduler, 0, value),
            0x0400_0209 => self.interrupts[1]
                .master_enable
                .write(&mut self.scheduler, 1, value),
            0x0400_020A => self.interrupts[1]
                .master_enable
                .write(&mut self.scheduler, 2, value),
            0x0400_020B => self.interrupts[1]
                .master_enable
                .write(&mut self.scheduler, 3, value),
            0x0400_0210 => self.interrupts[1]
                .enable
                .write(&mut self.scheduler, 0, value),
            0x0400_0211 => self.interrupts[1]
                .enable
                .write(&mut self.scheduler, 1, value),
            0x0400_0212 => self.interrupts[1]
                .enable
                .write(&mut self.scheduler, 2, value),
            0x0400_0213 => self.interrupts[1]
                .enable
                .write(&mut self.scheduler, 3, value),
            0x0400_0214 => self.interrupts[1]
                .request
                .write(&mut self.scheduler, 0, value),
            0x0400_0215 => self.interrupts[1]
                .request
                .write(&mut self.scheduler, 1, value),
            0x0400_0216 => self.interrupts[1]
                .request
                .write(&mut self.scheduler, 2, value),
            0x0400_0217 => self.interrupts[1]
                .request
                .write(&mut self.scheduler, 3, value),
            0x0400_0240..=0x0400_0246 => self.gpu.vram.write_vram_cnt(addr as usize & 0xF, value),
            0x0400_0247 => self.wramcnt.write(&mut self.scheduler, 0, value),
            0x0400_0248..=0x0400_0249 => self
                .gpu
                .vram
                .write_vram_cnt((addr as usize & 0xF) - 1, value),
            0x0400_0280..=0x0400_0283 => {
                self.div
                    .cnt
                    .write(&mut self.scheduler, addr as usize & 0xF, value)
            }
            0x0400_0290..=0x0400_0297 => {
                self.div
                    .write_numer(&mut self.scheduler, addr as usize & 0x7, value)
            }
            0x0400_0298..=0x0400_029F => {
                self.div
                    .write_denom(&mut self.scheduler, addr as usize & 0x7, value)
            }
            0x0400_02A0..=0x0400_02A7 => (), // Div result registers are read-only
            0x0400_02A8..=0x0400_02AF => (), // Div result registers are read-only
            0x0400_02B0..=0x0400_02B3 => {
                self.sqrt
                    .cnt
                    .write(&mut self.scheduler, addr as usize & 0xF, value)
            }
            0x0400_02B4..=0x0400_02B7 => (), // Sqrt result register is read-only
            0x0400_02B8..=0x0400_02BF => {
                self.sqrt
                    .write_param(&mut self.scheduler, addr as usize & 0x7, value)
            }
            0x0400_0300 => self.postflg9 = (self.postflg9 & !0x02 | value & 0x02) | (value & 0x1), // Only bit 1 is writable
            0x0400_0301..=0x0400_0303 => (), // Other Parts of POSTFLG
            0x0400_0304 => self.gpu.powcnt1.write(&mut self.scheduler, 0, value),
            0x0400_0305 => self.gpu.powcnt1.write(&mut self.scheduler, 1, value),
            0x0400_0306 => self.gpu.powcnt1.write(&mut self.scheduler, 2, value),
            0x0400_0307 => self.gpu.powcnt1.write(&mut self.scheduler, 3, value),
            0x0400_0320..=0x0400_06A3 => {
                self.gpu
                    .engine3d
                    .write_register(&mut self.scheduler, addr, value)
            }
            0x0400_1000..=0x0400_1003 => {
                self.gpu
                    .engine_b
                    .write_register(&mut self.scheduler, addr, value)
            }
            0x0400_1004..=0x0400_1007 => (),
            0x0400_1008..=0x0400_105F => {
                self.gpu
                    .engine_b
                    .write_register(&mut self.scheduler, addr, value)
            }
            0x0400_1060..=0x0400_106B => (),
            0x0400_106C => self
                .gpu
                .engine_b
                .master_bright
                .write(&mut self.scheduler, 0, value),
            0x0400_106D => self
                .gpu
                .engine_b
                .master_bright
                .write(&mut self.scheduler, 1, value),
            0x0400_106E => self
                .gpu
                .engine_b
                .master_bright
                .write(&mut self.scheduler, 2, value),
            0x0400_106F => self
                .gpu
                .engine_b
                .master_bright
                .write(&mut self.scheduler, 3, value),
            _ => warn!(
                "Ignoring ARM9 IO Register Write 0x{:08X} = {:02X}",
                addr, value
            ),
        }
    }

    pub(super) fn arm9_write_io16(&mut self, addr: u32, value: u16) {
        self.arm9_write_io8(addr + 0, (value >> 0) as u8);
        self.arm9_write_io8(addr + 1, (value >> 8) as u8);
    }

    pub(super) fn arm9_write_io32(&mut self, addr: u32, value: u32) {
        match addr {
            0x0400_0188 => self.ipc_fifo_send(false, value),
            0x0400_0400..=0x0400_043F => self.write_geometry_fifo(value),
            0x0400_0440..=0x0400_05CB => self.write_geometry_command(addr, value),
            _ => {
                self.arm9_write_io8(addr + 0, (value >> 0) as u8);
                self.arm9_write_io8(addr + 1, (value >> 8) as u8);
                self.arm9_write_io8(addr + 2, (value >> 16) as u8);
                self.arm9_write_io8(addr + 3, (value >> 24) as u8);
            }
        }
    }

    fn write_geometry_fifo(&mut self, value: u32) {
        self.gpu.engine3d.write_geometry_fifo(value);
    }

    fn write_geometry_command(&mut self, addr: u32, value: u32) {
        self.gpu.engine3d.write_geometry_command(addr, value);
        self.check_geometry_command_fifo();
    }
}
