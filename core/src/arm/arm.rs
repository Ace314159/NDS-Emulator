use super::{instructions::InstructionHandler, registers::Mode, ARM, HW};

use crate::hw::AccessType;

impl<const IS_ARM9: bool> ARM<IS_ARM9> {
    pub(super) fn fill_arm_instr_buffer(&mut self, hw: &mut HW) {
        self.regs[15] &= !0x3;
        self.instr_buffer[0] = self.read::<u32>(hw, AccessType::S, self.regs[15] & !0x3);
        self.regs[15] = self.regs[15].wrapping_add(4);

        self.instr_buffer[1] = self.read::<u32>(hw, AccessType::S, self.regs[15] & !0x3);
    }

    pub(super) fn emulate_arm_instr(&mut self, hw: &mut HW) {
        let instr = self.instr_buffer[0];
        {
            trace!("{:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} \
            {:08X} {:08X} {:08X} {:08X} cpsr: {:08X} | {:08X}",
            self.regs[0], self.regs[1], self.regs[2], self.regs[3], self.regs[4], self.regs[5], self.regs[6],
            self.regs[7], self.regs[8], self.regs[9], self.regs[10], self.regs[11], self.regs[12],
            self.regs[13], self.regs[14], self.regs[15], self.regs.cpsr(), instr);
        }
        self.instr_buffer[0] = self.instr_buffer[1];
        self.regs[15] = self.regs[15].wrapping_add(4);

        if self.should_exec((instr >> 28) & 0xF) {
            self.arm_lut[((instr as usize) >> 16 & 0xFF0) | ((instr as usize) >> 4 & 0xF)](
                self, hw, instr,
            );
        } else {
            self.instruction_prefetch::<u32>(hw, AccessType::S);
        }
    }

    // ARM.3: Branch and Exchange (BX, BLX)
    fn branch_and_exchange<const L: bool>(&mut self, hw: &mut HW, instr: u32) {
        self.instruction_prefetch::<u32>(hw, AccessType::N);
        if IS_ARM9 {
            if L {
                // BLX
                assert_eq!(instr >> 4 & 0xF, 0b0011);
                self.regs.set_lr(self.regs[15].wrapping_sub(4));
            } else {
                assert_eq!(instr >> 4 & 0xF, 0b0001)
            }
        }
        // BX
        self.regs[15] = self.regs[instr & 0xF];
        if self.regs[15] & 0x1 != 0 {
            self.regs[15] -= 1;
            self.regs.set_t(true);
            self.fill_thumb_instr_buffer(hw);
        } else {
            self.fill_arm_instr_buffer(hw)
        }
    }

    // ARM.4: Branch and Branch with Link (B, BL, BLX)
    fn branch_branch_with_link<const L: bool>(&mut self, hw: &mut HW, instr: u32) {
        let offset = instr & 0xFF_FFFF;
        let offset = if (offset >> 23) == 1 {
            0xFF00_0000 | offset
        } else {
            offset
        };
        self.instruction_prefetch::<u32>(hw, AccessType::N);

        if IS_ARM9 && instr >> 28 == 0xF {
            // BLX
            self.regs.set_lr(self.regs[15].wrapping_sub(4));
            self.regs[15] = self.regs[15]
                .wrapping_add(offset << 2)
                .wrapping_add((L as u32) * 2); // L acts as H
            self.regs.set_t(true);
            self.fill_thumb_instr_buffer(hw);
        } else {
            if L {
                self.regs.set_lr(self.regs[15].wrapping_sub(4))
            } // Branch with Link
            self.regs[15] = self.regs[15].wrapping_add(offset << 2);
            self.fill_arm_instr_buffer(hw);
        }
    }

    // ARM.5: Data Processing
    fn data_proc<
        const I: bool,
        const OP3: bool,
        const OP2: bool,
        const OP1: bool,
        const OP0: bool,
        const S: bool,
        const SHIFT1: bool,
        const SHIFT0: bool,
        const R: bool,
    >(
        &mut self,
        hw: &mut HW,
        instr: u32,
    ) {
        let immediate_op2 = I;
        let change_status = S;
        let mut temp_inc_pc = false;
        let opcode = (OP3 as u32) << 3 | (OP2 as u32) << 2 | (OP1 as u32) << 1 | (OP0 as u32);
        let dest_reg = (instr >> 12) & 0xF;
        let (change_status, special_change_status) = if dest_reg == 15 && change_status {
            (false, true)
        } else {
            (change_status, false)
        };
        let op2 = if immediate_op2 {
            let shift = (instr >> 8) & 0xF;
            let operand = instr & 0xFF;
            if (opcode < 0x5 || opcode > 0x7) && shift != 0 {
                self.shift(3, operand, shift * 2, true, change_status)
            } else {
                operand.rotate_right(shift * 2)
            }
        } else {
            let shift_by_reg = R;
            let shift = if shift_by_reg {
                assert_eq!((instr >> 7) & 0x1, 0);
                self.regs[15] = self.regs[15].wrapping_add(4); // Temp inc
                temp_inc_pc = true;
                self.regs[(instr >> 8) & 0xF] & 0xFF
            } else {
                (instr >> 7) & 0x1F
            };
            let shift_type = (SHIFT1 as u32) << 1 | (SHIFT0 as u32);
            let op2 = self.regs[instr & 0xF];
            // TODO: I Cycle occurs too early
            self.shift(
                shift_type,
                op2,
                shift,
                !shift_by_reg,
                change_status && (opcode < 0x5 || opcode > 0x7),
            )
        };
        let op1 = self.regs[(instr >> 16) & 0xF];
        let result = match opcode {
            0x0 | 0x8 => op1 & op2,                         // AND and TST
            0x1 | 0x9 => op1 ^ op2,                         // EOR and TEQ
            0x2 | 0xA => self.sub(op1, op2, change_status), // SUB and CMP
            0x3 => self.sub(op2, op1, change_status),       // RSB
            0x4 | 0xB => self.add(op1, op2, change_status), // ADD and CMN
            0x5 => self.adc(op1, op2, change_status),       // ADC
            0x6 => self.sbc(op1, op2, change_status),       // SBC
            0x7 => self.sbc(op2, op1, change_status),       // RSC
            0xC => op1 | op2,                               // ORR
            0xD => op2,                                     // MOV
            0xE => op1 & !op2,                              // BIC
            0xF => !op2,                                    // MVN
            _ => unreachable!(),
        };
        if change_status {
            self.regs.set_z(result == 0);
            self.regs.set_n(result & 0x8000_0000 != 0);
        } else if special_change_status {
            self.regs.restore_cpsr()
        } else {
            assert_eq!(opcode & 0xC != 0x8, true)
        }
        let mut clocked = false;
        if opcode & 0xC != 0x8 {
            if dest_reg == 15 {
                clocked = true;
                self.instruction_prefetch::<u32>(hw, AccessType::N);
                self.regs[15] = result;
                if self.regs.get_t() {
                    self.fill_thumb_instr_buffer(hw)
                } else {
                    self.fill_arm_instr_buffer(hw)
                }
            } else {
                self.regs[dest_reg] = result
            }
        }
        if !clocked {
            if temp_inc_pc {
                self.regs[15] = self.regs[15].wrapping_sub(4)
            } // Dec after temp inc
            self.instruction_prefetch::<u32>(hw, AccessType::S);
        }
    }

    // ARM.6: PSR Transfer (MRS, MSR)
    fn psr_transfer<const I: bool, const P: bool, const L: bool>(
        &mut self,
        hw: &mut HW,
        instr: u32,
    ) {
        assert_eq!(instr >> 26 & 0b11, 0b00);
        let immediate_operand = I;
        assert_eq!(instr >> 23 & 0b11, 0b10);
        let use_spsr = P;
        let msr = L;
        assert_eq!(instr >> 20 & 0b1, 0b0);
        self.instruction_prefetch::<u32>(hw, AccessType::S);

        if msr {
            let mut mask = 0u32;
            if instr >> 19 & 0x1 != 0 {
                mask |= 0xFF000000
            } // Flags
            if instr >> 18 & 0x1 != 0 {
                mask |= 0x00FF0000
            } // Status
            if instr >> 17 & 0x1 != 0 {
                mask |= 0x0000FF00
            } // Extension
            if self.regs.get_mode() != Mode::USR && instr >> 16 & 0x1 != 0 {
                mask |= 0x000000FF
            } // Control
            assert_eq!(instr >> 12 & 0xF, 0xF);
            let operand = if immediate_operand {
                let shift = instr >> 8 & 0xF;
                (instr & 0xFF).rotate_right(shift * 2)
            } else {
                assert_eq!(instr >> 4 & 0xFF, 0);
                self.regs[instr & 0xF]
            };
            let set = |value: &mut u32| *value = *value & !mask | operand & mask;
            if use_spsr {
                set(self.regs.spsr_mut());
                self.regs.update_spsr_mode();
            } else {
                self.regs.save_banked();
                set(&mut self.regs.cpsr_mut());
                self.regs.update_cpsr_mode();
                self.regs.load_banked(self.regs.get_mode());
            }
        } else {
            assert_eq!(immediate_operand, false);
            self.regs[instr >> 12 & 0xF] = if use_spsr {
                self.regs.spsr()
            } else {
                self.regs.cpsr()
            };
            assert_eq!(instr & 0xFFF, 0);
        }
    }

    // ARM.7: Multiply and Multiply-Accumulate (MUL, MLA)
    fn mul_mula<const A: bool, const S: bool>(&mut self, hw: &mut HW, instr: u32) {
        assert_eq!(instr >> 22 & 0x3F, 0b000000);
        let accumulate = A;
        let change_status = S;
        let dest_reg = instr >> 16 & 0xF;
        let op1_reg = instr >> 12 & 0xF;
        let op1 = self.regs[op1_reg];
        let op2 = self.regs[instr >> 8 & 0xF];
        assert_eq!(instr >> 4 & 0xF, 0b1001);
        let op3 = self.regs[instr & 0xF];

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
        self.regs[dest_reg] = result;
    }

    // ARM.8: Multiply Long and Multiply-Accumulate Long (MULL, MLAL)
    fn mul_long<const U: bool, const A: bool, const S: bool>(&mut self, hw: &mut HW, instr: u32) {
        assert_eq!(instr >> 23 & 0x1F, 0b00001);
        let signed = U;
        let accumulate = A;
        let change_status = S;
        let src_dest_reg_high = instr >> 16 & 0xF;
        let src_dest_reg_low = instr >> 12 & 0xF;
        let op1 = self.regs[instr >> 8 & 0xF];
        assert_eq!(instr >> 4 & 0xF, 0b1001);
        let op2 = self.regs[instr & 0xF];

        self.instruction_prefetch::<u32>(hw, AccessType::S);
        self.internal();
        self.inc_mul_clocks(op1 as u32, signed);
        let result = if signed {
            (op1 as i32 as u64).wrapping_mul(op2 as i32 as u64)
        } else {
            (op1 as u64) * (op2 as u64)
        }
        .wrapping_add(if accumulate {
            self.internal();
            (self.regs[src_dest_reg_high] as u64) << 32 | self.regs[src_dest_reg_low] as u64
        } else {
            0
        });
        if change_status {
            self.regs.set_n(result & 0x8000_0000_0000_0000 != 0);
            self.regs.set_z(result == 0);
        }
        self.regs[src_dest_reg_low] = (result >> 0) as u32;
        self.regs[src_dest_reg_high] = (result >> 32) as u32;
    }

    // ARM.9: Single Data Transfer (LDR, STR)
    fn single_data_transfer<
        const I: bool,
        const P: bool,
        const U: bool,
        const B: bool,
        const W: bool,
        const L: bool,
    >(
        &mut self,
        hw: &mut HW,
        instr: u32,
    ) {
        assert_eq!(instr >> 26 & 0b11, 0b01);
        let shifted_reg_offset = I;
        let pre_offset = P;
        let add_offset = U;
        let transfer_byte = B;
        let mut write_back = W || !pre_offset;
        let load = L;
        let base_reg = instr >> 16 & 0xF;
        let base = self.regs[base_reg];
        let src_dest_reg = instr >> 12 & 0xF;
        self.instruction_prefetch::<u32>(hw, AccessType::N);

        let offset = if shifted_reg_offset {
            let shift = instr >> 7 & 0x1F;
            let shift_type = instr >> 5 & 0x3;
            assert_eq!(instr >> 4 & 0x1, 0);
            let offset_reg = instr & 0xF;
            assert_ne!(offset_reg, 15);
            let operand = self.regs[offset_reg];
            self.shift(shift_type, operand, shift, true, false)
        } else {
            instr & 0xFFF
        };

        let mut exec = |addr| {
            if load {
                let access_type = if src_dest_reg == 15 {
                    AccessType::N
                } else {
                    AccessType::S
                };
                let value = if transfer_byte {
                    self.read::<u8>(hw, access_type, addr) as u32
                } else {
                    self.read::<u32>(hw, access_type, addr & !0x3)
                        .rotate_right((addr & 0x3) * 8)
                };
                self.internal();
                self.regs[src_dest_reg] = value;
                if src_dest_reg == base_reg {
                    write_back = false
                }
                if src_dest_reg == 15 {
                    if IS_ARM9 && self.regs[15] & 0x1 != 0 {
                        self.regs[15] -= 1;
                        self.regs.set_t(true);
                        self.fill_thumb_instr_buffer(hw);
                    } else {
                        self.fill_arm_instr_buffer(hw);
                    }
                }
            } else {
                let value = self.regs[src_dest_reg];
                let value = if src_dest_reg == 15 {
                    value.wrapping_add(4)
                } else {
                    value
                };
                if transfer_byte {
                    self.write::<u8>(hw, AccessType::N, addr, value as u8);
                } else {
                    self.write::<u32>(hw, AccessType::N, addr & !0x3, value);
                }
            }
        };
        let offset_applied = if add_offset {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };
        if pre_offset {
            exec(offset_applied);
            if write_back {
                self.regs[base_reg] = offset_applied
            }
        } else {
            // TOOD: Take into account privilege of access
            let force_non_privileged_access = instr >> 21 & 0x1 != 0;
            assert_eq!(force_non_privileged_access, false);
            // Write back is not done if src_reg == base_reg
            exec(base);
            if write_back {
                self.regs[base_reg] = offset_applied
            }
        }
    }

    // ARM.10: Halfword and Signed Data Transfer (STRH,LDRH,LDRSB,LDRSH)
    fn halfword_and_signed_data_transfer<
        const P: bool,
        const U: bool,
        const I: bool,
        const W: bool,
        const L: bool,
        const S: bool,
        const H: bool,
    >(
        &mut self,
        hw: &mut HW,
        instr: u32,
    ) {
        assert_eq!(instr >> 25 & 0x7, 0b000);
        let pre_offset = P;
        let add_offset = U;
        let immediate_offset = I;
        let mut write_back = W || !pre_offset;
        let load = L;
        let base_reg = instr >> 16 & 0xF;
        let base = self.regs[base_reg];
        let src_dest_reg = instr >> 12 & 0xF;
        let offset_hi = instr >> 8 & 0xF;
        assert_eq!(instr >> 7 & 0x1, 1);
        let signed = S;
        let halfword = H;
        let opcode = (signed as u8) << 1 | (halfword as u8);
        assert_eq!(instr >> 4 & 0x1, 1);
        let offset_low = instr & 0xF;
        self.instruction_prefetch::<u32>(hw, AccessType::N);

        let offset = if immediate_offset {
            offset_hi << 4 | offset_low
        } else {
            assert_eq!(offset_hi, 0);
            self.regs[offset_low]
        };

        let mut exec = |addr| {
            if load {
                if src_dest_reg == base_reg {
                    write_back = false
                }
                let access_type = if src_dest_reg == 15 {
                    AccessType::N
                } else {
                    AccessType::S
                };
                let value = if IS_ARM9 {
                    match opcode {
                        1 => self.read::<u16>(hw, access_type, addr & !0x1) as u32,
                        2 => self.read::<u8>(hw, access_type, addr) as i8 as u32,
                        3 => self.read::<u16>(hw, access_type, addr & !0x1) as i16 as u32,
                        _ => unreachable!(),
                    }
                } else {
                    match opcode {
                        1 => (self.read::<u16>(hw, access_type, addr & !0x1) as u32)
                            .rotate_right((addr & 0x1) * 8),
                        2 => self.read::<u8>(hw, access_type, addr) as i8 as u32,
                        3 if addr & 0x1 == 1 => self.read::<u8>(hw, access_type, addr) as i8 as u32,
                        3 => self.read::<u16>(hw, access_type, addr) as i16 as u32,
                        _ => unreachable!(),
                    }
                };
                self.internal();
                self.regs[src_dest_reg] = value;
                if src_dest_reg == 15 {
                    self.fill_arm_instr_buffer(hw)
                }
            } else {
                let addr = addr & !0x1;
                assert!(IS_ARM9 || opcode == 1);
                if opcode == 2 || opcode == 3 {
                    assert!(src_dest_reg % 2 == 0 && src_dest_reg != 14); // Rd is a multiple of 2 and cannot be 14
                    assert!(addr & 0x7 == 0); // Addr must be double-word aligned
                                              // For STRD, Rm != Rd and Rm != Rd + 1
                    if opcode == 3 && !immediate_offset {
                        assert!(offset_low != src_dest_reg && offset_low != src_dest_reg + 1)
                    }
                }
                match opcode {
                    1 => self.write::<u16>(hw, AccessType::N, addr, self.regs[src_dest_reg] as u16),
                    2 => {
                        // LDRD
                        let value1 = self.read::<u32>(hw, AccessType::N, addr);
                        let value2 = self.read::<u32>(hw, AccessType::S, addr + 4);
                        self.regs[src_dest_reg] = value1;
                        self.regs[src_dest_reg + 1] = value2;
                    }
                    3 => {
                        // STRD
                        assert!(base_reg != src_dest_reg && base_reg != src_dest_reg + 1); // Rn != Rd and Rn != Rd + 1
                        self.write::<u32>(hw, AccessType::N, addr, self.regs[src_dest_reg]);
                        self.write::<u32>(hw, AccessType::N, addr + 4, self.regs[src_dest_reg + 1]);
                    }
                    _ => unreachable!(),
                }
            }
        };
        let offset_applied = if add_offset {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };
        if pre_offset {
            exec(offset_applied);
            if write_back {
                self.regs[base_reg] = offset_applied
            }
        } else {
            exec(base);
            assert_eq!(instr >> 24 & 0x1 != 0, false);
            // Write back is not done if src_reg == base_reg
            if write_back {
                self.regs[base_reg] = offset_applied
            }
        }
    }

    // ARM.11: Block Data Transfer (LDM,STM)
    fn block_data_transfer<
        const P: bool,
        const U: bool,
        const S: bool,
        const W: bool,
        const L: bool,
    >(
        &mut self,
        hw: &mut HW,
        instr: u32,
    ) {
        assert_eq!(instr >> 25 & 0x7, 0b100);
        let add_offset = U;
        let pre_offset = P ^ !add_offset;
        let psr_force_usr = S;
        let write_back = W;
        let load = L;
        let base_reg = instr >> 16 & 0xF;
        assert_ne!(base_reg, 0xF);
        let base = self.regs[base_reg];
        let base_offset = base & 0x3;
        let base = base - base_offset;
        let mut r_list = (instr & 0xFFFF) as u16;
        let write_back = write_back && (IS_ARM9 || !(load && r_list & (1 << base_reg) != 0));
        let actual_mode = self.regs.get_mode();
        if psr_force_usr && !(load && r_list & (1 << 15) != 0) {
            self.regs.set_mode(Mode::USR)
        }

        self.instruction_prefetch::<u32>(hw, AccessType::N);
        let mut loaded_pc = false;
        let num_regs = r_list.count_ones();
        let start_addr = if add_offset {
            base
        } else {
            base.wrapping_sub(num_regs * 4)
        };
        let mut addr = start_addr;
        let final_addr = if add_offset {
            addr + 4 * num_regs
        } else {
            start_addr
        } + base_offset;
        let (final_addr, inc_amount) = if !IS_ARM9 && num_regs == 0 {
            (final_addr + 0x40, 0x40)
        } else {
            (final_addr, 4)
        };
        let mut calc_addr = || {
            if pre_offset {
                addr += inc_amount;
                addr
            } else {
                let old_addr = addr;
                addr += inc_amount;
                old_addr
            }
        };
        let mut exec = |addr, reg, last_access| {
            if load {
                let value = self.read::<u32>(hw, AccessType::S, addr);
                self.regs[reg] = value;
                if !IS_ARM9 && write_back {
                    self.regs[base_reg] = final_addr;
                }
                if last_access {
                    self.internal()
                }
                if reg == 15 {
                    if psr_force_usr {
                        self.regs.restore_cpsr()
                    }
                    loaded_pc = true;
                    self.next_access_type = AccessType::N;
                    if IS_ARM9 && self.regs[15] & 0x1 != 0 {
                        self.regs[15] -= 1;
                        self.regs.set_t(true);
                        self.fill_thumb_instr_buffer(hw);
                    } else {
                        self.fill_arm_instr_buffer(hw);
                    }
                }
            } else {
                let value = self.regs[reg];
                let access_type = if last_access {
                    AccessType::N
                } else {
                    AccessType::S
                };
                self.write::<u32>(
                    hw,
                    access_type,
                    addr,
                    if reg == 15 {
                        value.wrapping_add(4)
                    } else {
                        value
                    },
                );
                if !IS_ARM9 && write_back {
                    self.regs[base_reg] = final_addr;
                }
            }
        };
        if num_regs == 0 {
            if IS_ARM9 && write_back {
                let value = if add_offset {
                    final_addr + 0x40
                } else {
                    final_addr - 0x40
                };
                self.regs[base_reg] = value;
            } else if !IS_ARM9 {
                exec(start_addr, 15, true);
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
            if IS_ARM9 {
                let write_back = if write_back && original_r_list & (1 << base_reg) != 0 && load {
                    // reg is the last register loaded
                    original_r_list.count_ones() == 1 || base_reg != reg
                } else {
                    write_back
                };
                if write_back {
                    self.regs[base_reg] = final_addr
                }
            }
        }

        self.regs.set_mode(actual_mode);
    }

    // ARM.12: Single Data Swap (SWP)
    fn single_data_swap<const B: bool>(&mut self, hw: &mut HW, instr: u32) {
        assert_eq!(instr >> 23 & 0x1F, 0b00010);
        let byte = B;
        assert_eq!(instr >> 20 & 0x3, 0b00);
        let base = self.regs[instr >> 16 & 0xF];
        let dest_reg = instr >> 12 & 0xF;
        assert_eq!(instr >> 4 & 0xFF, 0b00001001);
        let src_reg = instr & 0xF;
        let src = self.regs[src_reg];

        self.instruction_prefetch::<u32>(hw, AccessType::N);
        let value = if byte {
            let value = self.read::<u8>(hw, AccessType::N, base) as u32;
            self.write::<u8>(hw, AccessType::S, base, src as u8);
            value
        } else {
            let value = self
                .read::<u32>(hw, AccessType::N, base & !0x3)
                .rotate_right((base & 0x3) * 8);
            self.write::<u32>(hw, AccessType::S, base & !0x3, src);
            value
        };
        self.regs[dest_reg] = value;
        self.internal();
    }

    // ARM.13: Software Interrupt (SWI)
    fn arm_software_interrupt(&mut self, hw: &mut HW, instr: u32) {
        assert_eq!(instr >> 24 & 0xF, 0b1111);
        self.instruction_prefetch::<u32>(hw, AccessType::N);
        self.regs.change_mode(Mode::SVC);
        self.regs.set_lr(self.regs[15].wrapping_sub(4));
        self.regs.set_i(true);
        let interrupt_base = if IS_ARM9 { hw.cp15.interrupt_base() } else { 0 };
        self.regs[15] = interrupt_base | 0x8;
        self.fill_arm_instr_buffer(hw);
    }

    // ARM.14: Coprocessor Data Operations (CDP)
    // ARM.15: Coprocessor Data Transfers (LDC,STC)
    fn coprocessor(&mut self, _hw: &mut HW, _instr: u32) {
        unimplemented!("Coprocessor not implemented!");
    }

    // ARM.17: Undefined Instruction
    fn undefined_instr_arm(&mut self, _hw: &mut HW, _instr: u32) {
        unimplemented!("ARM.17: Undefined Instruction not implemented!");
    }
}

// TODO: Implement this only for ARM9
impl<const IS_ARM9: bool> ARM<IS_ARM9> {
    fn signed_half_mul<const OPH: bool, const OPL: bool, const Y: bool, const X: bool>(
        &mut self,
        hw: &mut HW,
        instr: u32,
    ) {
        assert_eq!(instr >> 23 & 0x1F, 0b00010);
        assert_eq!(instr >> 7 & 0x1, 1);
        assert_eq!(instr >> 4 & 0x1, 0);
        // TODO: Take into account interlock
        self.instruction_prefetch::<u32>(hw, AccessType::S);
        let opcode = (OPH as u8) << 1 | (OPL as u8);
        let dest_reg = instr >> 16 & 0xF;
        let accumulate_reg = instr >> 12 & 0xF;
        let accumulate = self.regs[accumulate_reg];
        let operand1 = self.regs[instr >> 8 & 0xF];
        let operand2 = self.regs[instr & 0xF];
        let get_half = |value: u32, top| {
            if top {
                (value >> 16) as u16 as i16
            } else {
                value as u16 as i16
            }
        };
        let result = match opcode {
            0b00 => {
                // SMLA
                let product = (get_half(operand2, X) as i32 * get_half(operand1, Y) as i32) as i32;
                let (result, overflowed) = product.overflowing_add(accumulate as i32);
                if overflowed {
                    self.regs.set_q(true)
                }
                result as u32
            }
            0b01 => {
                // SMLAW/SMULW
                let product = ((operand2 as i32 as u64)
                    .wrapping_mul(get_half(operand1, Y) as i32 as u64)
                    >> 16) as i32;
                if X {
                    // SMULW
                    product as u32
                } else {
                    // SMLAW
                    let (product, overflowed) = product.overflowing_add(accumulate as i32);
                    if overflowed {
                        self.regs.set_q(true)
                    }
                    product as u32
                }
            }
            0b10 => {
                // SMLAL
                self.internal();
                let product = (get_half(operand2, X) as i32 * get_half(operand1, Y) as i32) as u64;
                let hi_reg = dest_reg;
                let lo_reg = accumulate_reg;
                let hi_lo = (self.regs[hi_reg] as u64) << 32 | self.regs[lo_reg] as u64;
                let result = product.wrapping_add(hi_lo);
                self.regs[lo_reg] = result as u32;
                (result >> 32) as u32 // setes dest_reg which is hi_reg
            }
            0b11 => {
                // SMUL
                (get_half(operand2, X) as i32 * get_half(operand1, Y) as i32) as u32
            }
            _ => unreachable!(),
        };
        self.regs[dest_reg] = result;
    }

    // ARM.16: Coprocessor Register Transfers (MRC, MCR)
    fn coprocessor_register_transfers<
        const C_OP2: bool,
        const C_OP1: bool,
        const C_OP0: bool,
        const OP: bool,
        const CP2: bool,
        const CP1: bool,
        const CP0: bool,
    >(
        &mut self,
        hw: &mut HW,
        instr: u32,
    ) {
        // TODO: Do Timing
        self.instruction_prefetch::<u32>(hw, AccessType::S);
        assert_ne!(instr >> 28, 0xF); // TODO: Implement 2 variants
        assert_eq!(instr >> 24 & 0xF, 0b1110);
        let cp_op = (C_OP2 as u8) << 2 | (C_OP1 as u8) << 1 | (C_OP0 as u8);
        let cp_n = instr >> 8 & 0xF;
        if cp_op != 0 || cp_n != 15 {
            return;
        }
        let cp_src_dest_reg = instr >> 16 & 0xF;
        let arm_src_dest_reg = instr >> 12 & 0xF;
        let cp_info = (CP2 as u32) << 2 | (CP1 as u32) << 1 | (CP0 as u32);
        assert_eq!(instr >> &4 & 0x1, 1);
        let cp_operand_reg = instr & 0xF;
        if OP {
            // MRC
            self.regs[arm_src_dest_reg] = hw.cp15.read(cp_src_dest_reg, cp_operand_reg, cp_info);
        } else {
            // MCR
            hw.cp15.write(
                cp_src_dest_reg,
                cp_operand_reg,
                cp_info,
                self.regs[arm_src_dest_reg],
            );
            // Check if we modified ITCM or DTCM
            if cp_src_dest_reg == 9 && cp_operand_reg == 1 && cp_info <= 1 {
                // TODO: Only update ITCM and DTCM portions
                hw.init_arm9_page_tables()
            }
        }
    }

    // ARM.X: Count Leading Zeros
    fn count_leading_zeros(&mut self, hw: &mut HW, instr: u32) {
        assert_eq!(instr >> 16 & 0xFFF, 0b0001_0110_1111);
        assert_eq!(instr >> 4 & 0xFF, 0b1111_0001);
        self.instruction_prefetch::<u32>(hw, AccessType::S);
        let dest_reg = instr >> 12 & 0xF;
        let src = self.regs[instr & 0xF];
        self.regs[dest_reg] = src.leading_zeros();
    }

    // ARM.X: QADD/QSUB
    fn qalu<const D: bool, const OP: bool>(&mut self, hw: &mut HW, instr: u32) {
        assert_eq!(instr >> 24 & 0xF, 0b0001);
        assert_eq!(instr >> 4 & 0xFF, 0b0000_0101);
        // TODO: Take into account interlock
        self.instruction_prefetch::<u32>(hw, AccessType::S);
        let src2 = self.regs[instr >> 16 & 0xF] as i32;
        let dest_reg = instr >> 12 & 0xF;
        let src1 = self.regs[instr & 0xF] as i32;
        let (src2, q1) = if D {
            (src2.saturating_mul(2), src2.checked_mul(2).is_none())
        } else {
            (src2, false)
        };
        let (result, q2) = if OP {
            (src1.saturating_sub(src2), src1.checked_sub(src2).is_none())
        } else {
            (src1.saturating_add(src2), src1.checked_add(src2).is_none())
        };
        self.regs[dest_reg] = result as u32;
        if q1 || q2 {
            self.regs.set_q(true)
        }
    }
}

pub(super) fn gen_lut<const IS_ARM9: bool>() -> [InstructionHandler<u32, IS_ARM9>; 4096] {
    // Bits 0-3 of opcode = Bits 4-7 of instr
    // Bits 4-11 of opcode = Bits Bits 20-27 of instr
    let mut lut: [InstructionHandler<u32, IS_ARM9>; 4096] = [ARM::undefined_instr_arm; 4096];

    for opcode in 0..4096 {
        let skeleton = ((opcode & 0xFF0) << 16) | ((opcode & 0xF) << 4);
        lut[opcode] = if skeleton & 0b1111_1111_0000_0000_0000_1101_0000
            == 0b0001_0010_0000_0000_0000_0001_0000
        {
            compose_instr_handler!(branch_and_exchange, skeleton, 5)
        } else if skeleton & 0b1111_1100_0000_0000_0000_1111_0000
            == 0b0000_0000_0000_0000_0000_1001_0000
        {
            compose_instr_handler!(mul_mula, skeleton, 21, 20)
        } else if skeleton & 0b1111_1000_0000_0000_0000_1111_0000
            == 0b0000_1000_0000_0000_0000_1001_0000
        {
            compose_instr_handler!(mul_long, skeleton, 22, 21, 20)
        } else if skeleton & 0b1111_1000_0000_0000_1111_1111_0000
            == 0b0001_0000_0000_0000_0000_1001_0000
        {
            compose_instr_handler!(single_data_swap, skeleton, 22)
        } else if skeleton & 0b1110_0000_0000_0000_0000_1001_0000
            == 0b0000_0000_0000_0000_0000_1001_0000
        {
            compose_instr_handler!(
                halfword_and_signed_data_transfer,
                skeleton,
                24,
                23,
                22,
                21,
                20,
                6,
                5
            )
        } else if skeleton & 0b1111_1111_0000_0000_0000_1111_0000
            == 0b0001_0110_0000_0000_0000_0001_0000
        {
            ARM::count_leading_zeros
        } else if skeleton & 0b1111_1001_0000_0000_0000_1111_0000
            == 0b0001_0000_0000_0000_0000_0101_0000
        {
            compose_instr_handler!(qalu, skeleton, 22, 21)
        } else if skeleton & 0b1111_1001_0000_0000_0000_1001_0000
            == 0b0001_0000_0000_0000_0000_1000_0000
        {
            compose_instr_handler!(signed_half_mul, skeleton, 22, 21, 6, 5)
        } else if skeleton & 0b1101_1001_0000_0000_0000_0000_0000
            == 0b0001_0000_0000_0000_0000_0000_0000
        {
            compose_instr_handler!(psr_transfer, skeleton, 25, 22, 21)
        } else if skeleton & 0b1100_0000_0000_0000_0000_0000_0000
            == 0b0000_0000_0000_0000_0000_0000_0000
        {
            compose_instr_handler!(data_proc, skeleton, 25, 24, 23, 22, 21, 20, 6, 5, 4)
        } else if skeleton & 0b1100_0000_0000_0000_0000_0000_0000
            == 0b0100_0000_0000_0000_0000_0000_0000
        {
            compose_instr_handler!(single_data_transfer, skeleton, 25, 24, 23, 22, 21, 20)
        } else if skeleton & 0b1110_0000_0000_0000_0000_0000_0000
            == 0b1000_0000_0000_0000_0000_0000_0000
        {
            compose_instr_handler!(block_data_transfer, skeleton, 24, 23, 22, 21, 20)
        } else if skeleton & 0b1110_0000_0000_0000_0000_0000_0000
            == 0b1010_0000_0000_0000_0000_0000_0000
        {
            compose_instr_handler!(branch_branch_with_link, skeleton, 24)
        } else if skeleton & 0b1111_0000_0000_0000_0000_0000_0000
            == 0b1111_0000_0000_0000_0000_0000_0000
        {
            ARM::arm_software_interrupt
        } else if skeleton & 0b1110_0000_0000_0000_0000_0000_0000
            == 0b1100_0000_0000_0000_0000_0000_0000
        {
            ARM::coprocessor
        } else if skeleton & 0b1111_0000_0000_0000_0000_0001_0000
            == 0b1110_0000_0000_0000_0000_0000_0000
        {
            ARM::coprocessor
        } else if skeleton & 0b1111_0000_0000_0000_0000_0001_0000
            == 0b1110_0000_0000_0000_0000_0001_0000
        {
            compose_instr_handler!(
                coprocessor_register_transfers,
                skeleton,
                23,
                22,
                21,
                20,
                7,
                6,
                5
            )
        } else {
            assert_eq!(
                skeleton & 0b1110_0000_0000_0000_0000_0001_0000,
                0b0110_0000_0000_0000_0000_0001_0000
            );
            ARM::undefined_instr_arm
        };
    }

    lut
}
