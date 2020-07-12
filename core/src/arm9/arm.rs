use super::{
    ARM9, HW,
    instructions::{InstructionFlag, InstructionHandler, InstrFlagSet, InstrFlagClear},
    registers::{Reg, Mode}
};


use crate::hw::AccessType;

impl ARM9 {
    pub(super) fn fill_arm_instr_buffer(&mut self, hw: &mut HW) {
        self.regs.pc &= !0x3;
        self.instr_buffer[0] = self.read::<u32>(hw, AccessType::S, self.regs.pc & !0x3);
        self.regs.pc = self.regs.pc.wrapping_add(4);

        self.instr_buffer[1] = self.read::<u32>(hw, AccessType::S, self.regs.pc & !0x3);
    }

    pub(super) fn emulate_arm_instr(&mut self, hw: &mut HW) {
        let instr = self.instr_buffer[0];
        {
            use Reg::*;
            trace!("{:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} \
            {:08X} {:08X} {:08X} {:08X} cpsr: {:08X} | {:08X}",
            self.regs.get_reg(R0), self.regs.get_reg(R1), self.regs.get_reg(R2), self.regs.get_reg(R3),
            self.regs.get_reg(R4), self.regs.get_reg(R5), self.regs.get_reg(R6), self.regs.get_reg(R7),
            self.regs.get_reg(R8), self.regs.get_reg(R9), self.regs.get_reg(R10), self.regs.get_reg(R11),
            self.regs.get_reg(R12), self.regs.get_reg(R13), self.regs.get_reg(R14), self.regs.get_reg(R15),
            self.regs.get_reg(CPSR), instr);
        }
        self.instr_buffer[0] = self.instr_buffer[1];
        self.regs.pc = self.regs.pc.wrapping_add(4);

        if self.should_exec((instr >> 28) & 0xF) {
            self.arm_lut[((instr as usize) >> 16 & 0xFF0) | ((instr as usize) >> 4 & 0xF)](self, hw, instr);
        } else {
            self.instruction_prefetch::<u32>(hw, AccessType::S);
        }
    }

    // ARM.3: Branch and Exchange (BX, BLX)
    fn branch_and_exchange<L: InstructionFlag>(&mut self, hw: &mut HW, instr: u32) {
        self.instruction_prefetch::<u32>(hw, AccessType::N);
        if L::bool() { // BLX
            assert_eq!(instr >> 4 & 0xF, 0b0011);
            self.regs.set_reg(Reg::R14, self.regs.pc.wrapping_sub(4));
        } else { assert_eq!(instr >> 4 & 0xF, 0b0011); } // BX
        self.regs.pc = self.regs.get_reg_i(instr & 0xF);
        if self.regs.pc & 0x1 != 0 {
            self.regs.pc -= 1;
            self.regs.set_t(true);
            self.fill_thumb_instr_buffer(hw);
        } else { self.fill_arm_instr_buffer(hw) }
    }

    // ARM.4: Branch and Branch with Link (B, BL, BLX)
    fn branch_branch_with_link<L: InstructionFlag>(&mut self, hw: &mut HW, instr: u32) {
        let offset = instr & 0xFF_FFFF;
        let offset = if (offset >> 23) == 1 { 0xFF00_0000 | offset } else { offset };
        self.instruction_prefetch::<u32>(hw, AccessType::N);

        if instr >> 28 == 0xF { // BLX
            self.regs.set_reg(Reg::R14, self.regs.pc.wrapping_sub(4));
            self.regs.pc = self.regs.pc.wrapping_add(offset << 2).wrapping_add(L::num() * 2); // L acts as H
            self.regs.set_t(true);
            self.fill_thumb_instr_buffer(hw);
        } else {
            if L::bool() { self.regs.set_reg(Reg::R14, self.regs.pc.wrapping_sub(4)) } // Branch with Link
            self.regs.pc = self.regs.pc.wrapping_add(offset << 2);
            self.fill_arm_instr_buffer(hw);
        }
    }

    // ARM.5: Data Processing
    fn data_proc<I: InstructionFlag, S: InstructionFlag>(&mut self, hw: &mut HW, instr: u32) {
        let immediate_op2 = I::bool();
        let change_status = S::bool();
        let mut temp_inc_pc = false;
        let opcode = (instr >> 21) & 0xF;
        let dest_reg = (instr >> 12) & 0xF;
        let (change_status, special_change_status) = if dest_reg == 15 && change_status {
            (false, true) } else { (change_status, false)
        };
        let op2 = if immediate_op2 {
            let shift = (instr >> 8) & 0xF;
            let operand = instr & 0xFF;
            if (opcode < 0x5 || opcode > 0x7) && shift != 0 {
                self.shift(3, operand, shift * 2, true, change_status)
            } else { operand.rotate_right(shift * 2) }
        } else {
            let shift_by_reg = (instr >> 4) & 0x1 != 0;
            let shift = if shift_by_reg {
                assert_eq!((instr >> 7) & 0x1, 0);
                self.regs.pc = self.regs.pc.wrapping_add(4); // Temp inc
                temp_inc_pc = true;
                self.regs.get_reg_i((instr >> 8) & 0xF) & 0xFF
            } else {
                (instr >> 7) & 0x1F
            };
            let shift_type = (instr >> 5) & 0x3;
            let op2 = self.regs.get_reg_i(instr & 0xF);
            // TODO: I Cycle occurs too early
            self.shift(shift_type, op2, shift, !shift_by_reg,
                change_status && (opcode < 0x5 || opcode > 0x7))
        };
        let op1 = self.regs.get_reg_i((instr >> 16) & 0xF);
        let result = match opcode {
            0x0 | 0x8 => op1 & op2, // AND and TST
            0x1 | 0x9 => op1 ^ op2, // EOR and TEQ
            0x2 | 0xA => self.sub(op1, op2, change_status), // SUB and CMP
            0x3 => self.sub(op2, op1, change_status), // RSB
            0x4 | 0xB => self.add(op1, op2, change_status), // ADD and CMN
            0x5 => self.adc(op1, op2, change_status), // ADC
            0x6 => self.sbc(op1, op2, change_status), // SBC
            0x7 => self.sbc(op2, op1, change_status), // RSC
            0xC => op1 | op2, // ORR
            0xD => op2, // MOV
            0xE => op1 & !op2, // BIC
            0xF => !op2, // MVN
            _ => unreachable!(),
        };
        if change_status {
            self.regs.set_z(result == 0);
            self.regs.set_n(result & 0x8000_0000 != 0);
        } else if special_change_status { self.regs.set_reg(Reg::CPSR, self.regs.get_reg(Reg::SPSR)) }
        else { assert_eq!(opcode & 0xC != 0x8, true) }
        let mut clocked = false;
        if opcode & 0xC != 0x8 {
            if dest_reg == 15 {
                clocked = true;
                self.instruction_prefetch::<u32>(hw, AccessType::N);
                self.regs.pc = result;
                if self.regs.get_t() { self.fill_thumb_instr_buffer(hw) }
                else { self.fill_arm_instr_buffer(hw) }
            } else { self.regs.set_reg_i(dest_reg, result) }
        }
        if !clocked {
            if temp_inc_pc { self.regs.pc = self.regs.pc.wrapping_sub(4) } // Dec after temp inc
            self.instruction_prefetch::<u32>(hw, AccessType::S);
        }
    }

    // ARM.6: PSR Transfer (MRS, MSR)
    fn psr_transfer<I: InstructionFlag, P: InstructionFlag, L: InstructionFlag>(&mut self, hw: &mut HW, instr: u32) {
        assert_eq!(instr >> 26 & 0b11, 0b00);
        let immediate_operand = I::bool();
        assert_eq!(instr >> 23 & 0b11, 0b10);
        let status_reg = if P::bool() { Reg::SPSR } else { Reg::CPSR };
        let msr = L::bool();
        assert_eq!(instr >> 20 & 0b1, 0b0);
        self.instruction_prefetch::<u32>(hw, AccessType::S);

        if msr {
            let mut mask = 0u32;
            if instr >> 19 & 0x1 != 0 { mask |= 0xFF000000 } // Flags
            if instr >> 18 & 0x1 != 0 { mask |= 0x00FF0000 } // Status
            if instr >> 17 & 0x1 != 0 { mask |= 0x0000FF00 } // Extension
            if self.regs.get_mode() != Mode::USR && instr >> 16 & 0x1 != 0 { mask |= 0x000000FF } // Control
            assert_eq!(instr >> 12 & 0xF, 0xF);
            let operand = if immediate_operand {
                let shift = instr >> 8 & 0xF;
                (instr & 0xFF).rotate_right(shift * 2)
            } else {
                assert_eq!(instr >> 4 & 0xFF, 0);
                self.regs.get_reg_i(instr & 0xF)
            };
            let value = self.regs.get_reg(status_reg) & !mask | operand & mask;
            self.regs.set_reg(status_reg, value);
        } else {
            assert_eq!(immediate_operand, false);
            self.regs.set_reg_i(instr >> 12 & 0xF, self.regs.get_reg(status_reg));
            assert_eq!(instr & 0xFFF, 0);
        }
    }
    
    // ARM.7: Multiply and Multiply-Accumulate (MUL, MLA)
    fn mul_mula<A: InstructionFlag, S: InstructionFlag>(&mut self, hw: &mut HW, instr: u32) {
        assert_eq!(instr >> 22 & 0x3F, 0b000000);
        let accumulate = A::bool();
        let change_status = S::bool();
        let dest_reg = instr >> 16 & 0xF;
        let op1_reg = instr >> 12 & 0xF;
        let op1 = self.regs.get_reg_i(op1_reg);
        let op2 = self.regs.get_reg_i(instr >> 8 & 0xF);
        assert_eq!(instr >> 4 & 0xF, 0b1001);
        let op3 = self.regs.get_reg_i(instr & 0xF);
        
        self.instruction_prefetch::<u32>(hw, AccessType::S);
        self.inc_mul_clocks(op2, true);
        let result = if accumulate {
            self.internal();
            op2.wrapping_mul(op3).wrapping_add(op1)
        } else {
            assert_eq!(op1_reg, 0);
            op2.wrapping_mul(op3)
        };
        if change_status {
            self.regs.set_n(result & 0x8000_0000 != 0);
            self.regs.set_z(result == 0);
        }
        self.regs.set_reg_i(dest_reg, result);
    }

    // ARM.8: Multiply Long and Multiply-Accumulate Long (MULL, MLAL)
    fn mul_long<U: InstructionFlag, A: InstructionFlag, S: InstructionFlag>(&mut self, hw: &mut HW, instr: u32) {
        assert_eq!(instr >> 23 & 0x1F, 0b00001);
        let signed = U::bool();
        let accumulate = A::bool();
        let change_status = S::bool();
        let src_dest_reg_high = instr >> 16 & 0xF;
        let src_dest_reg_low = instr >> 12 & 0xF;
        let op1 = self.regs.get_reg_i(instr >> 8 & 0xF);
        assert_eq!(instr >> 4 & 0xF, 0b1001);
        let op2 = self.regs.get_reg_i(instr & 0xF);

        self.instruction_prefetch::<u32>(hw, AccessType::S);
        self.internal();
        self.inc_mul_clocks(op1 as u32, signed);
        let result = if signed { (op1 as i32 as u64).wrapping_mul(op2 as i32 as u64) }
        else { (op1 as u64) * (op2 as u64) }.wrapping_add(
        if accumulate {
            self.internal();
            (self.regs.get_reg_i(src_dest_reg_high) as u64) << 32 |
            self.regs.get_reg_i(src_dest_reg_low) as u64
        } else { 0 });
        if change_status {
            self.regs.set_n(result & 0x8000_0000_0000_0000 != 0);
            self.regs.set_z(result == 0);
        }
        self.regs.set_reg_i(src_dest_reg_low, (result >> 0) as u32);
        self.regs.set_reg_i(src_dest_reg_high, (result >> 32) as u32);
    }

    // ARM.9: Single Data Transfer (LDR, STR)
    fn single_data_transfer<I: InstructionFlag, P: InstructionFlag, U: InstructionFlag,
                            B: InstructionFlag, W: InstructionFlag, L: InstructionFlag>(&mut self, hw: &mut HW, instr: u32) {
        assert_eq!(instr >> 26 & 0b11, 0b01);
        let shifted_reg_offset = I::bool();
        let pre_offset = P::bool();
        let add_offset = U::bool();
        let transfer_byte = B::bool();
        let mut write_back = W::bool() || !pre_offset;
        let load = instr >> 20 & 0x1 != 0;
        let base_reg = instr >> 16 & 0xF;
        let base = self.regs.get_reg_i(base_reg);
        let src_dest_reg = instr >> 12 & 0xF;
        self.instruction_prefetch::<u32>(hw, AccessType::N);

        let offset = if shifted_reg_offset {
            let shift = instr >> 7 & 0x1F;
            let shift_type = instr >> 5 & 0x3;
            assert_eq!(instr >> 4 & 0x1, 0);
            let offset_reg = instr & 0xF;
            assert_ne!(offset_reg, 15);
            let operand = self.regs.get_reg_i(offset_reg);
            self.shift(shift_type, operand, shift, true, false)
        } else {
            instr & 0xFFF
        };

        let mut exec = |addr| if load {
            let access_type = if src_dest_reg == 15 { AccessType::N } else { AccessType::S };
            let value = if transfer_byte {
                self.read::<u8>(hw, access_type, addr) as u32
            } else {
                self.read::<u32>(hw, access_type, addr & !0x3).rotate_right((addr & 0x3) * 8)
            };
            self.internal();
            self.regs.set_reg_i(src_dest_reg, value);
            if src_dest_reg == base_reg { write_back = false }
            if src_dest_reg == 15 { self.fill_arm_instr_buffer(hw) }
        } else {
            let value = self.regs.get_reg_i(src_dest_reg);
            let value = if src_dest_reg == 15 { value.wrapping_add(4) } else { value };
            if transfer_byte {
                self.write::<u8>(hw, AccessType::N, addr, value as u8);
            } else {
                self.write::<u32>(hw, AccessType::N, addr & !0x3, value);
            }
        };
        let offset_applied = if add_offset { base.wrapping_add(offset) } else { base.wrapping_sub(offset) };
        if pre_offset {
            exec(offset_applied);
            if write_back { self.regs.set_reg_i(base_reg, offset_applied) }
        } else {
            // TOOD: Take into account privilege of access
            let force_non_privileged_access = instr >> 21 & 0x1 != 0;
            assert_eq!(force_non_privileged_access, false);
            // Write back is not done if src_reg == base_reg
            exec(base);
            if write_back { self.regs.set_reg_i(base_reg, offset_applied) }
        }
    }

    // ARM.10: Halfword and Signed Data Transfer (STRH,LDRH,LDRSB,LDRSH)
    fn halfword_and_signed_data_transfer<P: InstructionFlag, U: InstructionFlag, I: InstructionFlag, W: InstructionFlag,
        L: InstructionFlag, S: InstructionFlag, H: InstructionFlag>(&mut self, hw: &mut HW, instr: u32) {
        assert_eq!(instr >> 25 & 0x7, 0b000);
        let pre_offset = P::bool();
        let add_offset = U::bool();
        let immediate_offset = I::bool();
        let mut write_back = W::bool() || !pre_offset;
        let load = L::bool();
        let base_reg = instr >> 16 & 0xF;
        let base = self.regs.get_reg_i(base_reg);
        let src_dest_reg = instr >> 12 & 0xF;
        let offset_hi = instr >> 8 & 0xF;
        assert_eq!(instr >> 7 & 0x1, 1);
        let signed = S::bool();
        let halfword = H::bool();
        let opcode = (signed as u8) << 1 | (halfword as u8);
        assert_eq!(instr >> 4 & 0x1, 1);
        let offset_low = instr & 0xF;
        self.instruction_prefetch::<u32>(hw, AccessType::N);
        
        let offset = if immediate_offset { offset_hi << 4 | offset_low }
        else {
            assert_eq!(offset_hi, 0);
            self.regs.get_reg_i(offset_low)
        };
        
        let mut exec = |addr| if load {
            if src_dest_reg == base_reg { write_back = false }
            let access_type = if src_dest_reg == 15 { AccessType::N } else { AccessType::S };
            let value = match opcode {
                1 => self.read::<u16>(hw, access_type, addr & !0x1) as u32,
                2 => self.read::<u8>(hw, access_type, addr) as i8 as u32,
                3 => self.read::<u16>(hw, access_type, addr & !0x1) as i16 as u32,
                _ => unreachable!(),
            };
            self.internal();
            self.regs.set_reg_i(src_dest_reg, value);
            if src_dest_reg == 15 { self.fill_arm_instr_buffer(hw) }
        } else {
            assert_eq!(opcode, 1);
            let addr = addr & !0x1;
            let value = self.regs.get_reg_i(src_dest_reg);
            self.write::<u16>(hw, AccessType::N, addr, value as u16);
        };
        let offset_applied = if add_offset { base.wrapping_add(offset) } else { base.wrapping_sub(offset) };
        if pre_offset {
            exec(offset_applied);
            if write_back { self.regs.set_reg_i(base_reg, offset_applied) }
        } else {
            exec(base);
            assert_eq!(instr >> 24 & 0x1 != 0, false);
            // Write back is not done if src_reg == base_reg
            if write_back { self.regs.set_reg_i(base_reg, offset_applied) }
        }
    }

    // ARM.11: Block Data Transfer (LDM,STM)
    fn block_data_transfer<P: InstructionFlag, U: InstructionFlag, S: InstructionFlag, W: InstructionFlag, L: InstructionFlag>
        (&mut self, hw: &mut HW, instr: u32) {
        assert_eq!(instr >> 25 & 0x7, 0b100);
        let add_offset = U::bool();
        let pre_offset = P::bool() ^ !add_offset;
        let psr_force_usr = S::bool();
        let write_back = W::bool();
        let load = L::bool();
        let base_reg = instr >> 16 & 0xF;
        assert_ne!(base_reg, 0xF);
        let base = self.regs.get_reg_i(base_reg);
        let base_offset = base & 0x3;
        let base = base - base_offset;
        let mut r_list = (instr & 0xFFFF) as u16;
        let actual_mode = self.regs.get_mode();
        if psr_force_usr && !(load && r_list & 0x80 != 0) { self.regs.set_mode(Mode::USR) }

        self.instruction_prefetch::<u32>(hw, AccessType::N);
        let mut loaded_pc = false;
        let num_regs = r_list.count_ones();
        let start_addr = if add_offset { base } else { base.wrapping_sub(num_regs * 4) };
        let mut addr = start_addr;
        let final_addr = if add_offset { addr + 4 * num_regs } else { start_addr } + base_offset;
        let mut calc_addr = || if pre_offset { addr += 4; addr }
        else { let old_addr = addr; addr += 4; old_addr };
        let mut exec = |addr, reg, last_access| if load {
            let value = self.read::<u32>(hw, AccessType::S, addr);
            self.regs.set_reg_i(reg, value);
            if last_access { self.internal() }
            if reg == 15 {
                if psr_force_usr { self.regs.restore_cpsr() }
                loaded_pc = true;
                self.next_access_type = AccessType::N;
                if self.regs.pc & 0x1 != 0 {
                    self.regs.pc -= 1;
                    self.regs.set_t(true);
                    self.fill_thumb_instr_buffer(hw);
                } else {
                    self.fill_arm_instr_buffer(hw);
                }
            }
        } else {
            let value = self.regs.get_reg_i(reg);
            let access_type = if last_access { AccessType::N } else { AccessType::S };
            self.write::<u32>(hw, access_type, addr, if reg == 15 { value.wrapping_add(4) } else { value });
            
        };
        if num_regs == 0 {
            if write_back {
                let value = if add_offset { final_addr + 0x40 } else { final_addr - 0x40 };
                self.regs.set_reg_i(base_reg, value);
            }
        } else {
            let mut reg = 0;
            let original_r_list = r_list;
            while r_list != 0x1 {
                if r_list & 0x1 != 0 {
                    exec(calc_addr(), reg, false);
                }
                reg += 1;
                r_list >>= 1;
            }
            exec(calc_addr(), reg, true);
            let write_back = if original_r_list & (1 << base_reg) != 0 && load {
                // reg is the last register loaded
                original_r_list.count_ones() == 1 || base_reg != reg
            } else { write_back };
            if write_back { self.regs.set_reg_i(base_reg, final_addr) }
        }

        self.regs.set_mode(actual_mode);
    }

    // ARM.12: Single Data Swap (SWP)
    fn single_data_swap<B: InstructionFlag>(&mut self, hw: &mut HW, instr: u32) {
        assert_eq!(instr >> 23 & 0x1F, 0b00010);
        let byte = B::bool();
        assert_eq!(instr >> 20 & 0x3, 0b00);
        let base = self.regs.get_reg_i(instr >> 16 & 0xF);
        let dest_reg = instr >> 12 & 0xF;
        assert_eq!(instr >> 4 & 0xFF, 0b00001001);
        let src_reg = instr & 0xF;
        let src = self.regs.get_reg_i(src_reg);

        self.instruction_prefetch::<u32>(hw, AccessType::N);
        let value = if byte {
            let value = self.read::<u8>(hw, AccessType::N, base) as u32;
            self.write::<u8>(hw, AccessType::S, base, src as u8);
            value
        } else {
            let value = self.read::<u32>(hw, AccessType::N, base & !0x3).rotate_right((base & 0x3) * 8);
            self.write::<u32>(hw, AccessType::S, base & !0x3, src);
            value
        };
        self.regs.set_reg_i(dest_reg, value);
        self.internal();
    }

    // ARM.13: Software Interrupt (SWI)
    fn arm_software_interrupt(&mut self, hw: &mut HW, instr: u32) {
        assert_eq!(instr >> 24 & 0xF, 0b1111);
        self.instruction_prefetch::<u32>(hw, AccessType::N);
        self.regs.change_mode(Mode::SVC);
        self.regs.set_reg(Reg::R14, self.regs.pc.wrapping_sub(4));
        self.regs.set_i(true);
        self.regs.pc = 0x8;
        self.fill_arm_instr_buffer(hw);
    }

    // ARM.14: Coprocessor Data Operations (CDP)
    // ARM.15: Coprocessor Data Transfers (LDC,STC)
    fn coprocessor(&mut self, _hw: &mut HW, _instr: u32) {
        unimplemented!("Coprocessor not implemented!");
    }

    // ARM.16: Coprocessor Register Transfers (MRC, MCR)
    fn coprocessor_register_transfers<COp2: InstructionFlag, COp1: InstructionFlag, COp0: InstructionFlag,
        Op: InstructionFlag, CP2: InstructionFlag, CP1: InstructionFlag, CP0: InstructionFlag>
        (&mut self, hw: &mut HW, instr: u32) {
        // TODO: Do Timing
        self.instruction_prefetch::<u32>(hw, AccessType::S);
        // TODO: Figure out difference beween MRC2 and MCR2
        assert_eq!(instr >> 24 & 0xF, 0b1110);
        let cp_op = COp2::num() << 2 | COp1::num() << 1 | COp0::num();
        let cp_n = instr >> 8 & 0xF;
        if cp_op != 0 || cp_n != 15 { return }
        let cp_src_dest_reg = instr >> 16 & 0xF;
        let arm_src_dest_reg = instr >> 12 & 0xF;
        let cp_info = CP2::num() << 2 | CP1::num() << 1 | CP0::num();
        assert_eq!(instr >> & 4 & 0x1, 1);
        let cp_operand_reg = instr & 0xF;
        if Op::bool() { // MRC
            self.regs.set_reg_i(arm_src_dest_reg, hw.cp15.read(cp_src_dest_reg, cp_operand_reg, cp_info));
        } else { // MCR
            hw.cp15.write(cp_src_dest_reg, cp_operand_reg, cp_info, self.regs.get_reg_i(arm_src_dest_reg));
        }
    }

    // ARM.17: Undefined Instruction
    fn undefined_instr_arm(&mut self, _hw: &mut HW, _instr: u32) {
        unimplemented!("ARM.17: Undefined Instruction not implemented!");
    }
}

pub(super) fn gen_lut() -> [InstructionHandler<u32>; 4096] {
    // Bits 0-3 of opcode = Bits 4-7 of instr
    // Bits 4-11 of opcode = Bits Bits 20-27 of instr
    let mut lut: [InstructionHandler<u32>; 4096] = [ARM9::undefined_instr_arm; 4096];

    for opcode in 0..4096 {
        let skeleton = ((opcode & 0xFF0) << 16) | ((opcode & 0xF) << 4);
        lut[opcode] = if skeleton & 0b1111_1111_0000_0000_0000_1101_0000 == 0b0001_0010_0000_0000_0000_0001_0000 {
            compose_instr_handler!(branch_and_exchange, skeleton, 5)
        } else if skeleton & 0b1111_1100_0000_0000_0000_1111_0000 == 0b0000_0000_0000_0000_0000_1001_0000 {
            compose_instr_handler!(mul_mula, skeleton, 21, 20)
        } else if skeleton & 0b1111_1000_0000_0000_0000_1111_0000 == 0b0000_1000_0000_0000_0000_1001_0000 {
            compose_instr_handler!(mul_long, skeleton, 22, 21, 20)
        } else if skeleton & 0b1111_1000_0000_0000_1111_1111_0000 == 0b0001_0000_0000_0000_0000_1001_0000 {
            compose_instr_handler!(single_data_swap, skeleton, 22)
        } else if skeleton & 0b1110_0000_0000_0000_0000_1001_0000 == 0b0000_0000_0000_0000_0000_1001_0000 {
            compose_instr_handler!(halfword_and_signed_data_transfer, skeleton, 24, 23, 22, 21, 20, 6, 5)
        } else if skeleton & 0b1101_1001_0000_0000_0000_0000_0000 == 0b0001_0000_0000_0000_0000_0000_0000 {
            compose_instr_handler!(psr_transfer, skeleton, 25, 22, 21)
        } else if skeleton & 0b1100_0000_0000_0000_0000_0000_0000 == 0b0000_0000_0000_0000_0000_0000_0000 {
            compose_instr_handler!(data_proc, skeleton, 25, 20)
        } else if skeleton & 0b1100_0000_0000_0000_0000_0000_0000 == 0b0100_0000_0000_0000_0000_0000_0000 {
            compose_instr_handler!(single_data_transfer, skeleton, 25, 24, 23, 22, 21, 20)
        } else if skeleton & 0b1110_0000_0000_0000_0000_0000_0000 == 0b1000_0000_0000_0000_0000_0000_0000 {
            compose_instr_handler!(block_data_transfer, skeleton, 24, 23, 22, 21, 20)
        } else if skeleton & 0b1110_0000_0000_0000_0000_0000_0000 == 0b1010_0000_0000_0000_0000_0000_0000 {
            compose_instr_handler!(branch_branch_with_link, skeleton, 24)
        } else if skeleton & 0b1111_0000_0000_0000_0000_0000_0000 == 0b1111_0000_0000_0000_0000_0000_0000 {
            ARM9::arm_software_interrupt
        } else if skeleton & 0b1110_0000_0000_0000_0000_0000_0000 == 0b1100_0000_0000_0000_0000_0000_0000 {
            ARM9::coprocessor
        } else if skeleton & 0b1111_0000_0000_0000_0000_0001_0000 == 0b1110_0000_0000_0000_0000_0000_0000 {
            ARM9::coprocessor
        } else if skeleton & 0b1111_0000_0000_0000_0000_0001_0000 == 0b1110_0000_0000_0000_0000_0001_0000 {
            compose_instr_handler!(coprocessor_register_transfers, skeleton, 23, 22, 21, 20, 7, 6, 5)
        } else {
            assert_eq!(skeleton & 0b1110_0000_0000_0000_0000_0001_0000, 0b0110_0000_0000_0000_0000_0001_0000);
            ARM9::undefined_instr_arm
        };
    }

    lut
}
