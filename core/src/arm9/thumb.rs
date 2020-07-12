use super::{
    ARM9, HW,
    instructions::{InstructionFlag, InstructionHandler, InstrFlagSet, InstrFlagClear},
    registers::{Reg, Mode}
};

use crate::hw::AccessType;

impl ARM9 {
    pub(super) fn fill_thumb_instr_buffer(&mut self, hw: &mut HW) {
        self.regs.pc &= !0x1;
        self.instr_buffer[0] = self.read::<u16>(hw, AccessType::S, self.regs.pc & !0x1) as u32;
        self.regs.pc = self.regs.pc.wrapping_add(2);

        self.instr_buffer[1] = self.read::<u16>(hw, AccessType::S, self.regs.pc & !0x1) as u32;
    }

    pub(super) fn emulate_thumb_instr(&mut self, hw: &mut HW) {
        let instr = self.instr_buffer[0] as u16;
        {
            use Reg::*;
            trace!("{:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} \
            {:08X} {:08X} {:08X} {:08X} cpsr: {:08X} | {}",
            self.regs.get_reg(R0), self.regs.get_reg(R1), self.regs.get_reg(R2), self.regs.get_reg(R3),
            self.regs.get_reg(R4), self.regs.get_reg(R5), self.regs.get_reg(R6), self.regs.get_reg(R7),
            self.regs.get_reg(R8), self.regs.get_reg(R9), self.regs.get_reg(R10), self.regs.get_reg(R11),
            self.regs.get_reg(R12), self.regs.get_reg(R13), self.regs.get_reg(R14), self.regs.get_reg(R15),
            self.regs.get_reg(CPSR), if instr & 0b1111_1000_0000_0000 == 0b1111_0000_0000_0000 {
                format!("{:04X}{:04X}", instr, self.instr_buffer[1])
            } else { format!("    {:04X}", instr) });
        }
        self.instr_buffer[0] = self.instr_buffer[1];
        self.regs.pc = self.regs.pc.wrapping_add(2);

        self.thumb_lut[(instr >> 8) as usize](self, hw, instr);
    }
    
    // THUMB.1: move shifted register
    fn move_shifted_reg<OpH: InstructionFlag, OpL: InstructionFlag>(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 13, 0b000);
        let opcode = OpH::num() << 1 | OpL::num();
        let offset = (instr >> 6 & 0x1F) as u32;
        let src = self.regs.get_reg_i((instr >> 3 & 0x7) as u32);
        let dest_reg = (instr & 0x7) as u32;
        assert_ne!(opcode, 0b11);
        let result = self.shift(opcode, src, offset, true, true);

        self.regs.set_n(result & 0x8000_0000 != 0);
        self.regs.set_z(result == 0);
        self.regs.set_reg_i(dest_reg, result);
        self.instruction_prefetch::<u16>(hw, AccessType::S);
    }

    // THUMB.2: add/subtract
    fn add_sub<I: InstructionFlag, SUB: InstructionFlag>(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 11, 0b00011);
        let immediate = I::bool();
        let sub = SUB::bool();
        let operand = (instr >> 6 & 0x7) as u32;
        let operand = if immediate { operand } else { self.regs.get_reg_i(operand) };
        let src = self.regs.get_reg_i((instr >> 3 & 0x7) as u32);
        let dest_reg = (instr & 0x7) as u32;

        let result = if sub {
            self.sub(src, operand, true)
        } else {
            self.add(src, operand, true)
        };
        self.regs.set_reg_i(dest_reg, result);
        self.instruction_prefetch::<u16>(hw, AccessType::S);
    }

    // THUMB.3: move/compare/add/subtract immediate
    fn immediate<OpH: InstructionFlag, OpL: InstructionFlag, Rd2: InstructionFlag, Rd1: InstructionFlag, Rd0: InstructionFlag>
        (&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 13, 0b001);
        let opcode = OpH::num() << 1 | OpL::num();
        let dest_reg = Rd2::num() << 2 | Rd1::num() << 1 | Rd0::num();
        let immediate = (instr & 0xFF) as u32;
        let op1 = self.regs.get_reg_i(dest_reg as u32);
        let result = match opcode {
            0b00 => immediate, // MOV
            0b01 => self.sub(op1, immediate, true), // CMP
            0b10 => self.add(op1, immediate, true), // ADD
            0b11 => self.sub(op1, immediate, true), // SUB
            _ => unreachable!(),
        };
        self.regs.set_z(result == 0);
        self.regs.set_n(result & 0x8000_0000 != 0);

        if opcode != 0b01 { self.regs.set_reg_i(dest_reg as u32, result) }
        self.instruction_prefetch::<u16>(hw, AccessType::S);
    }

    // THUMB.4: ALU operations
    fn alu(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 10 & 0x3F, 0b010000);
        self.instruction_prefetch::<u16>(hw, AccessType::S);
        let opcode = instr >> 6 & 0xF;
        let src = self.regs.get_reg_i((instr >> 3 & 0x7) as u32);
        let dest_reg = (instr & 0x7) as u32;
        let dest = self.regs.get_reg_i(dest_reg);
        let result = match opcode {
            0x0 => dest & src, // AND
            0x1 => dest ^ src, // XOR 
            0x2 => self.shift(0, dest, src & 0xFF, false, true), // LSL
            0x3 => self.shift(1, dest, src & 0xFF, false, true), // LSR
            0x4 => self.shift(2, dest, src & 0xFF, false, true), // ASR
            0x5 => self.adc(dest, src, true), // ADC
            0x6 => self.sbc(dest, src, true), // SBC
            0x7 => self.shift(3, dest, src & 0xFF, false, true), // ROR
            0x8 => dest & src, // TST
            0x9 => self.sub(0, src, true), // NEG
            0xA => self.sub(dest, src, true), // CMP
            0xB => self.add(dest, src, true), // CMN
            0xC => dest | src, // ORR
            0xD => { self.inc_mul_clocks(dest, true); dest.wrapping_mul(src) }, // MUL
            0xE => dest & !src, // BIC
            0xF => !src, // MVN
            _ => unreachable!(),
        };
        self.regs.set_n(result & 0x8000_0000 != 0);
        self.regs.set_z(result == 0);

        if ![0x8, 0xA, 0xB].contains(&opcode) { self.regs.set_reg_i(dest_reg, result) }
    }

    // THUMB.5: Hi register operations/branch exchange
    fn hi_reg_bx<OpH: InstructionFlag, OpL: InstructionFlag>(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 10, 0b010001);
        let opcode = OpH::num() << 1 | OpL::num();
        let dest_reg_msb = instr >> 7 & 0x1;
        let src_reg_msb = instr >> 6 & 0x1;
        let src = self.regs.get_reg_i((src_reg_msb << 3 | instr >> 3 & 0x7) as u32);
        let dest_reg = (dest_reg_msb << 3 | instr & 0x7) as u32;
        let dest = self.regs.get_reg_i(dest_reg);
        let result = match opcode {
            0b00 => self.add(dest,src, false), // ADD
            0b01 => self.sub(dest, src, true), // CMP
            0b10 => src,
            0b11 => {
                assert_eq!(dest_reg_msb, 0);
                self.instruction_prefetch::<u16>(hw, AccessType::N);
                self.regs.pc = src;
                if src & 0x1 != 0 {
                    self.regs.pc = self.regs.pc & !0x1;
                    self.fill_thumb_instr_buffer(hw);
                } else {
                    self.regs.pc = self.regs.pc & !0x2;
                    self.regs.set_t(false);
                    self.fill_arm_instr_buffer(hw);
                }
                return
            },
            _ => unreachable!(),
        };
        if opcode & 0x1 == 0 { self.regs.set_reg_i(dest_reg, result) }
        if dest_reg == 15 {
            self.instruction_prefetch::<u16>(hw, AccessType::N);
            self.fill_thumb_instr_buffer(hw);
        } else { self.instruction_prefetch::<u16>(hw, AccessType::S); }
    }

    // THUMB.6: load PC-relative
    fn load_pc_rel<Rd2: InstructionFlag, Rd1: InstructionFlag, Rd0: InstructionFlag>(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 11, 0b01001);
        let dest_reg = Rd2::num() << 2 | Rd1::num() << 1 | Rd0::num();
        let offset = (instr & 0xFF) as u32;
        let addr = (self.regs.pc & !0x2).wrapping_add(offset * 4);
        self.instruction_prefetch::<u16>(hw, AccessType::N);
        let value = self.read::<u32>(hw, AccessType::N, addr & !0x3).rotate_right((addr & 0x3) * 8);
        self.regs.set_reg_i(dest_reg, value);
        self.internal();
    }

    // THUMB.7: load/store with register offset
    fn load_store_reg_offset<OpH: InstructionFlag, OpL: InstructionFlag>(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 12, 0b0101);
        let opcode = OpH::num() << 1 | OpL::num(); 
        assert_eq!(instr >> 9 & 0x1, 0);
        let offset_reg = (instr >> 6 & 0x7) as u32;
        let base_reg = (instr >> 3 & 0x7) as u32;
        let addr = self.regs.get_reg_i(base_reg).wrapping_add(self.regs.get_reg_i(offset_reg));
        let src_dest_reg = (instr & 0x7) as u32;
        self.instruction_prefetch::<u16>(hw, AccessType::N);
        if opcode & 0b10 != 0 { // Load
            let value = if opcode & 0b01 != 0 {
                self.read::<u8>(hw, AccessType::S, addr) as u32 // LDRB
            } else {
                self.read::<u32>(hw, AccessType::S, addr & !0x3).rotate_right((addr & 0x3) * 8) // LDR
            };
            self.regs.set_reg_i(src_dest_reg, value);
            self.internal();
        } else { // Store
            if opcode & 0b01 != 0 { // STRB
                self.write::<u8>(hw, AccessType::N, addr, self.regs.get_reg_i(src_dest_reg) as u8);
            } else { // STR
                self.write::<u32>(hw, AccessType::N, addr & !0x3, self.regs.get_reg_i(src_dest_reg));
            }
        }
    }

    // THUMB.8: load/store sign-extended byte/halfword
    fn load_store_sign_ext<OpH: InstructionFlag, OpL: InstructionFlag>(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 12, 0b0101);
        let opcode = OpH::num() << 1 | OpL::num();
        assert_eq!(instr >> 9 & 0x1, 1);
        let offset_reg = (instr >> 6 & 0x7) as u32;
        let base_reg = (instr >> 3 & 0x7) as u32;
        let src_dest_reg = (instr & 0x7) as u32;
        let addr = self.regs.get_reg_i(base_reg).wrapping_add(self.regs.get_reg_i(offset_reg));

        self.instruction_prefetch::<u16>(hw, AccessType::N);
        if opcode == 0 { // STRH
            self.write::<u16>(hw, AccessType::N, addr & !0x1, self.regs.get_reg_i(src_dest_reg) as u16);
        } else { // Load
            let value = match opcode {
                1 => self.read::<u8>(hw, AccessType::S, addr) as i8 as u32,
                2 => self.read::<u16>(hw, AccessType::S, addr & !0x1) as u32,
                3 => self.read::<u16>(hw, AccessType::S, addr & !0x1) as i16 as u32,
                _ => unreachable!()
            };
            self.regs.set_reg_i(src_dest_reg, value);
            self.internal();
        }
    }

    // THUMB.9: load/store with immediate offset
    fn load_store_imm_offset<B: InstructionFlag, H: InstructionFlag>(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 13, 0b011);
        let byte = B::bool();
        let load = H::bool();
        let offset = (instr >> 6 & 0x1F) as u32;
        let base = self.regs.get_reg_i((instr >> 3 & 0x7) as u32);
        let src_dest_reg = (instr & 0x7) as u32;

        self.instruction_prefetch::<u16>(hw, AccessType::N);
        if load {
            // Is access width 1? Probably not, could be just bug in prev version
            let value = if byte {
                let addr = base.wrapping_add(offset);
                self.read::<u8>(hw, AccessType::S, addr) as u32
            } else {
                let addr = base.wrapping_add(offset << 2);
                self.read::<u32>(hw, AccessType::S, addr & !0x3).rotate_right((addr & 0x3) * 8)
            };
            self.regs.set_reg_i(src_dest_reg, value);
            self.internal();
        } else {
            let value = self.regs.get_reg_i(src_dest_reg);
            // Is access width 1? Probably not, could be just bug in prev version
            if byte {
                self.write::<u8>(hw, AccessType::N, base.wrapping_add(offset), value as u8);
            } else {
                self.write::<u32>(hw, AccessType::N, base.wrapping_add(offset << 2) & !0x3, value);
            }
        }
    }

    // THUMB.10: load/store halfword
    fn load_store_halfword<L: InstructionFlag>(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 12, 0b1000);
        let load = L::bool();
        let offset = (instr >> 6 & 0x1F) as u32;
        let base = self.regs.get_reg_i((instr >> 3 & 0x7) as u32);
        let src_dest_reg = (instr & 0x7) as u32;
        let addr = base + offset * 2;

        self.instruction_prefetch::<u16>(hw, AccessType::N);
        if load {
            let value = self.read::<u16>(hw, AccessType::S, addr & !0x1) as u32;
            self.regs.set_reg_i(src_dest_reg, value);
            self.internal();
        } else {
            self.write::<u16>(hw, AccessType::N, addr & !0x1, self.regs.get_reg_i(src_dest_reg) as u16);
        }
    }

    // THUMB.11: load/store SP-relative
    fn load_store_sp_rel<L: InstructionFlag, Rd2: InstructionFlag, Rd1: InstructionFlag, Rd0: InstructionFlag>
        (&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 12 & 0xF, 0b1001);
        let load = L::bool();
        let src_dest_reg = Rd2::num() << 2 | Rd1::num() << 1 | Rd0::num();
        let offset = (instr & 0xFF) * 4;
        let addr = self.regs.get_reg(Reg::R13).wrapping_add(offset as u32);
        self.instruction_prefetch::<u16>(hw, AccessType::N);
        if load {
            let value = self.read::<u32>(hw, AccessType::S, addr & !0x3).rotate_right((addr & 0x3) * 8);
            self.regs.set_reg_i(src_dest_reg, value);
            self.internal();
        } else {
            self.write::<u32>(hw, AccessType::N, addr & !0x3, self.regs.get_reg_i(src_dest_reg));
        }
    }

    // THUMB.12: get relative address
    fn get_rel_addr<SP: InstructionFlag, Rd2: InstructionFlag, Rd1: InstructionFlag, Rd0: InstructionFlag>
        (&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 12 & 0xF, 0b1010);
        let src = if SP::bool() { // SP
            self.regs.get_reg(Reg::R13)
        } else { // PC
            self.regs.pc & !0x2
        };
        let dest_reg = Rd2::num() << 2 | Rd1::num() << 1 | Rd0::num();
        let offset = (instr & 0xFF) as u32;
        self.regs.set_reg_i(dest_reg, src.wrapping_add(offset * 4));
        self.instruction_prefetch::<u16>(hw, AccessType::S);
    }

    // THUMB.13: add offset to stack pointer
    fn add_offset_sp(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 8 & 0xFF, 0b10110000);
        let sub = instr >> 7 & 0x1 != 0;
        let offset = ((instr & 0x7F) * 4) as u32;
        let sp = self.regs.get_reg(Reg::R13);
        let value = if sub { sp.wrapping_sub(offset) } else { sp.wrapping_add(offset) };
        self.regs.set_reg(Reg::R13, value);
        self.instruction_prefetch::<u16>(hw, AccessType::S);
    }

    // THUMB.14: push/pop registers
    fn push_pop_regs<L: InstructionFlag, R: InstructionFlag>(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 12 & 0xF, 0b1011);
        let pop = L::bool();
        assert_eq!(instr >> 9 & 0x3, 0b10);
        let pc_lr = R::bool();
        let mut r_list = (instr & 0xFF) as u8;
        self.instruction_prefetch::<u16>(hw, AccessType::N);
        if pop {
            let mut sp = self.regs.get_reg(Reg::R13);
            let mut stack_pop = |sp, last_access, reg: u32| {
                let value = self.read::<u32>(hw, AccessType::S, sp);
                self.regs.set_reg_i(reg, value);
                if last_access { self.internal() }
            };
            let mut reg = 0;
            while r_list != 0 {
                if r_list & 0x1 != 0 {
                    stack_pop(sp, r_list == 1 && !pc_lr, reg);
                    sp += 4;
                }
                reg += 1;
                r_list >>= 1;
            }
            if pc_lr {
                stack_pop(sp, true, 15);
                sp += 4;
                self.next_access_type = AccessType::N;
                if self.regs.pc & 0x1 != 0 {
                    self.regs.pc &= !0x1;
                    self.fill_thumb_instr_buffer(hw);
                } else {
                    self.regs.set_t(false);
                    self.regs.pc &= !0x3;
                    self.fill_arm_instr_buffer(hw);
                }
            }
            self.regs.set_reg(Reg::R13, sp);
        } else {
            let initial_sp = self.regs.get_reg(Reg::R13);
            let mut sp = self.regs.get_reg(Reg::R13).wrapping_sub(4 * (r_list.count_ones() + pc_lr as u32));
            self.regs.set_reg(Reg::R13, sp);
            let regs_copy = self.regs.clone();
            let mut stack_push = |sp, value, last_access| {
                self.write::<u32>(hw, AccessType::S, sp, value);
                if last_access { self.next_access_type = AccessType::N }
            };
            let mut reg = 0;
            while r_list != 0 {
                if r_list & 0x1 != 0 {
                    stack_push(sp, regs_copy.get_reg_i(reg), r_list == 0x1 && !pc_lr);
                    sp += 4;
                }
                reg += 1;
                r_list >>= 1;
            }
            if pc_lr { stack_push(sp, regs_copy.get_reg(Reg::R14), true); sp += 4}
            assert_eq!(initial_sp, sp);
        }
    }

    // THUMB.15: multiple load/store
    fn multiple_load_store<L: InstructionFlag, Rb2: InstructionFlag,
        Rb1: InstructionFlag, Rb0: InstructionFlag>(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 12, 0b1100);
        let load = L::bool();
        let base_reg = Rb2::num() << 2 | Rb1::num() << 1 | Rb0::num();
        let mut base = self.regs.get_reg_i(base_reg);
        let base_offset = base & 0x3;
        base -= base_offset;
        let mut r_list = (instr & 0xFF) as u8;
    
        self.instruction_prefetch::<u16>(hw, AccessType::N);
        let mut reg = 0;
        let mut first = true;
        let final_base = base.wrapping_add(4 * r_list.count_ones()) + base_offset;
        if !load { self.regs.pc = self.regs.pc.wrapping_add(2); }
        let mut exec = |reg, last_access| {
            let addr = base;
            base = base.wrapping_add(4);
            if load {
                let value = self.read::<u32>(hw, AccessType::S, addr);
                self.regs.set_reg_i(reg, value);
                if last_access { self.internal() }
            } else {
                self.write::<u32>(hw, AccessType::S, addr, self.regs.get_reg_i(reg));
                if last_access { self.next_access_type = AccessType::N }
                if first { self.regs.set_reg_i(base_reg, final_base); first = false }
            }
        };
        let mut write_back = true;
        if r_list == 0 {
            base = base.wrapping_add(0x3C + base_offset);
        } else {
            let original_r_list = r_list;
            while r_list != 0x1 {
                if r_list & 0x1 != 0 {
                    exec(reg, false);
                }
                reg += 1;
                r_list >>= 1;
            }
            write_back = if original_r_list & (1 << base_reg) != 0 && load {
                // reg is the last register loaded
                original_r_list.count_ones() == 1 || base_reg != reg
            } else { write_back };
            exec(reg, true);
        }
        //if load { io.inc_clock(Cycle::S, self.regs.pc.wrapping_add(2), 1) }
        if !load { self.regs.pc = self.regs.pc.wrapping_sub(2) }
        if write_back { self.regs.set_reg_i(base_reg, base + base_offset) }
    }

    // THUMB.16: conditional branch
    fn cond_branch<C3: InstructionFlag, C2: InstructionFlag, C1: InstructionFlag, C0: InstructionFlag>
        (&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 12, 0b1101);
        let condition = C3::num() << 3 | C2::num() << 2 | C1::num() << 1 | C0::num();
        assert_eq!(condition < 0xE, true);
        let offset = (instr & 0xFF) as i8 as u32;
        if self.should_exec(condition as u32) {
            self.instruction_prefetch::<u16>(hw, AccessType::N);
            self.regs.pc = self.regs.pc.wrapping_add(offset.wrapping_mul(2));
            self.fill_thumb_instr_buffer(hw);
        } else {
            self.instruction_prefetch::<u16>(hw, AccessType::S);
        }
    }

    // THUMB.17: software interrupt
    fn thumb_software_interrupt(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 8 & 0xFF, 0b11011111);
        self.instruction_prefetch::<u16>(hw, AccessType::N);
        self.regs.change_mode(Mode::SVC);
        self.regs.set_reg(Reg::R14, self.regs.pc.wrapping_sub(2));
        self.regs.set_t(false);
        self.regs.set_i(true);
        self.regs.pc = hw.cp15.interrupt_base() | 0x8;
        self.fill_arm_instr_buffer(hw);
    }

    // THUMB.18: unconditional branch
    fn uncond_branch(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 11, 0b11100);
        let offset = (instr & 0x7FF) as u32;
        let offset = if offset >> 10 & 0x1 != 0 { 0xFFFF_F800 | offset } else { offset };

        self.instruction_prefetch::<u16>(hw, AccessType::N);
        self.regs.pc = self.regs.pc.wrapping_add(offset << 1);
        self.fill_thumb_instr_buffer(hw);
    }

    // THUMB.19: long branch with link
    fn branch_with_link<H: InstructionFlag>(&mut self, hw: &mut HW, instr: u16) {
        assert_eq!(instr >> 12, 0xF);
        let offset = (instr & 0x7FF) as u32;
        if H::bool() { // Second Instruction
            self.instruction_prefetch::<u16>(hw, AccessType::N);
            let next_instr_pc = self.regs.pc.wrapping_sub(2);
            self.regs.pc = self.regs.get_reg(Reg::R14).wrapping_add(offset << 1);
            self.regs.set_reg(Reg::R14, next_instr_pc | 0x1);
            self.fill_thumb_instr_buffer(hw);
        } else { // First Instruction
            let offset = if offset >> 10 & 0x1 != 0 { 0xFFFF_F800 | offset } else { offset };
            assert_eq!(instr >> 11, 0b11110);
            self.regs.set_reg(Reg::R14, self.regs.pc.wrapping_add(offset << 12));
            self.instruction_prefetch::<u16>(hw, AccessType::S);
        }
    }

    fn undefined_instr_thumb(&mut self, _hw: &mut HW, _instr: u16) {
        panic!("Undefined Thumb Instruction!")
    }
}

pub(super) fn gen_lut() -> [InstructionHandler<u16>; 256] {
    // Bits 0-7 of opcode = Bits 16-31 of instr
    let mut lut: [InstructionHandler<u16>; 256] = [ARM9::undefined_instr_thumb; 256]; // Temp handler

    for opcode in 0..256 {
        let skeleton = opcode << 8;
        lut[opcode] = if opcode & 0b1111_1000 == 0b0001_1000 { compose_instr_handler!(add_sub, skeleton, 10, 9) }
        else if opcode & 0b1110_0000 == 0b0000_0000 { compose_instr_handler!(move_shifted_reg, skeleton, 12, 11) }
        else if opcode & 0b1110_0000 == 0b0010_0000 { compose_instr_handler!(immediate, skeleton, 12, 11, 10, 9, 8) }
        else if opcode & 0b1111_1100 == 0b0100_0000 { ARM9::alu }
        else if opcode & 0b1111_1100 == 0b0100_0100 { compose_instr_handler!(hi_reg_bx, skeleton, 9, 8) }
        else if opcode & 0b1111_1000 == 0b0100_1000 { compose_instr_handler!(load_pc_rel, skeleton, 10, 9, 8) }
        else if opcode & 0b1111_0010 == 0b0101_0000 { compose_instr_handler!(load_store_reg_offset, skeleton, 11, 10) }
        else if opcode & 0b1111_0010 == 0b0101_0010 { compose_instr_handler!(load_store_sign_ext, skeleton, 11, 10) }
        else if opcode & 0b1110_0000 == 0b0110_0000 { compose_instr_handler!(load_store_imm_offset, skeleton, 12, 11)}
        else if opcode & 0b1111_0000 == 0b1000_0000 { compose_instr_handler!(load_store_halfword, skeleton, 11) }
        else if opcode & 0b1111_0000 == 0b1001_0000 { compose_instr_handler!(load_store_sp_rel, skeleton, 11, 10, 9, 8) }
        else if opcode & 0b1111_0000 == 0b1010_0000 { compose_instr_handler!(get_rel_addr, skeleton, 11, 10, 9, 8) }
        else if opcode & 0b1111_1111 == 0b1011_0000 { ARM9::add_offset_sp }
        else if opcode & 0b1111_0110 == 0b1011_0100 { compose_instr_handler!(push_pop_regs, skeleton, 11, 8) }
        else if opcode & 0b1111_0000 == 0b1100_0000 { compose_instr_handler!(multiple_load_store, skeleton, 11, 10, 9, 8)}
        else if opcode & 0b1111_1111 == 0b1101_1111 { ARM9::thumb_software_interrupt }
        else if opcode & 0b1111_0000 == 0b1101_0000 { compose_instr_handler!(cond_branch, skeleton, 11, 10, 9, 8) }
        else if opcode & 0b1111_1000 == 0b1110_0000 { ARM9::uncond_branch }
        else if opcode & 0b1111_0000 == 0b1111_0000 { compose_instr_handler!(branch_with_link, skeleton, 11) }
        else { ARM9::undefined_instr_thumb };
    }

    lut
}
