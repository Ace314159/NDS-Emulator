#[macro_use]
mod instructions;
mod arm;
mod registers;
mod thumb;

use crate::hw::{AccessType, MemoryValue, HW};
use crate::num;
use registers::{Mode, RegValues};

pub struct ARM<const IS_ARM9: bool> {
    cycles_spent: usize,
    regs: RegValues,
    instr_buffer: [u32; 2],
    next_access_type: AccessType,

    condition_lut: [bool; 256],
    arm_lut: [instructions::InstructionHandler<u32, IS_ARM9>; 4096],
    thumb_lut: [instructions::InstructionHandler<u16, IS_ARM9>; 256],
}

impl<const IS_ARM9: bool> ARM<IS_ARM9> {
    pub fn new(hw: &mut HW, direct_boot: bool) -> ARM<IS_ARM9> {
        let mut cpu = ARM {
            cycles_spent: 0,
            regs: if direct_boot {
                RegValues::direct_boot(hw.init_arm9())
            } else {
                RegValues::new()
            },
            instr_buffer: [0; 2],
            next_access_type: AccessType::N,

            condition_lut: instructions::gen_condition_table(),
            arm_lut: arm::gen_lut(),
            thumb_lut: thumb::gen_lut(),
        };
        cpu.fill_arm_instr_buffer(hw);
        cpu
    }

    pub fn emulate_instr(&mut self, hw: &mut HW) -> usize {
        self.cycles_spent = 0;
        if self.regs.get_t() {
            self.emulate_thumb_instr(hw)
        } else {
            self.emulate_arm_instr(hw)
        }
        self.cycles_spent
    }

    pub fn read<T: MemoryValue>(&mut self, hw: &mut HW, access_type: AccessType, addr: u32) -> T {
        let value = hw.arm9_read::<T>(addr);
        self.cycles_spent += hw.arm9_get_access_time::<T>(self.next_access_type, addr);
        self.next_access_type = access_type;
        value
    }

    pub fn write<T: MemoryValue>(
        &mut self,
        hw: &mut HW,
        access_type: AccessType,
        addr: u32,
        value: T,
    ) {
        self.cycles_spent += hw.arm9_get_access_time::<T>(self.next_access_type, addr);
        self.next_access_type = access_type;
        hw.arm9_write::<T>(addr, value);
    }

    pub fn instruction_prefetch<T: MemoryValue>(&mut self, hw: &mut HW, access_type: AccessType) {
        // Internal Cycle merges with instruction prefetch
        // TODO: Increment PC here
        self.instr_buffer[1] =
            num::cast::<T, u32>(self.read::<T>(hw, access_type, self.regs[15])).unwrap();
    }

    pub fn internal(&mut self) {
        self.cycles_spent += 1;
        self.next_access_type = AccessType::N;
    }

    pub fn handle_irq(&mut self, hw: &mut HW) {
        if self.regs.get_i() || !hw.arm9_interrupts_requested() {
            return;
        }
        hw.cp15.arm9_halted = false;
        self.regs.change_mode(Mode::IRQ);
        let lr = if self.regs.get_t() {
            self.read::<u16>(hw, AccessType::N, self.regs[15]);
            self.regs[15].wrapping_sub(2).wrapping_add(4)
        } else {
            self.read::<u32>(hw, AccessType::N, self.regs[15]);
            self.regs[15].wrapping_sub(4).wrapping_add(4)
        };
        self.regs.set_lr(lr);
        self.regs.set_t(false);
        self.regs.set_i(true);
        self.regs[15] = hw.cp15.interrupt_base() | 0x18;
        self.fill_arm_instr_buffer(hw);
    }

    pub(self) fn should_exec(&self, condition: u32) -> bool {
        self.condition_lut[((self.regs.get_flags() & 0xF0) | condition) as usize]
    }

    pub(self) fn shift(
        &mut self,
        shift_type: u32,
        operand: u32,
        shift: u32,
        immediate: bool,
        change_status: bool,
    ) -> u32 {
        if immediate && shift == 0 {
            match shift_type {
                // LSL #0
                0 => operand,
                // LSR #32
                1 => {
                    if change_status {
                        self.regs.set_c(operand >> 31 != 0)
                    }
                    0
                }
                // ASR #32
                2 => {
                    let bit = operand >> 31 != 0;
                    if change_status {
                        self.regs.set_c(bit);
                    }
                    if bit {
                        0xFFFF_FFFF
                    } else {
                        0
                    }
                }
                // RRX #1
                3 => {
                    let new_c = operand & 0x1 != 0;
                    let op2 = (self.regs.get_c() as u32) << 31 | operand >> 1;
                    if change_status {
                        self.regs.set_c(new_c)
                    }
                    op2
                }
                _ => unreachable!(),
            }
        } else if shift > 31 {
            assert_eq!(immediate, false);
            if !immediate {
                self.internal()
            }
            match shift_type {
                // LSL
                0 => {
                    if change_status {
                        if shift == 32 {
                            self.regs.set_c(operand << (shift - 1) & 0x8000_0000 != 0)
                        } else {
                            self.regs.set_c(false)
                        }
                    }
                    0
                }
                // LSR
                1 => {
                    if change_status {
                        if shift == 32 {
                            self.regs.set_c(operand >> (shift - 1) & 0x1 != 0)
                        } else {
                            self.regs.set_c(false)
                        }
                    }
                    0
                }
                // ASR
                2 => {
                    let c = operand & 0x8000_0000 != 0;
                    if change_status {
                        self.regs.set_c(c)
                    }
                    if c {
                        0xFFFF_FFFF
                    } else {
                        0
                    }
                }
                // ROR
                3 => {
                    let shift = shift & 0x1F;
                    let shift = if shift == 0 { 0x20 } else { shift };
                    if change_status {
                        self.regs.set_c(operand >> (shift - 1) & 0x1 != 0)
                    }
                    operand.rotate_right(shift)
                }
                _ => unreachable!(),
            }
        } else {
            if !immediate {
                self.internal()
            }
            let change_status = change_status && shift != 0;
            match shift_type {
                // LSL
                0 => {
                    if change_status {
                        self.regs.set_c(operand << (shift - 1) & 0x8000_0000 != 0);
                    }
                    operand << shift
                }
                // LSR
                1 => {
                    if change_status {
                        self.regs.set_c(operand >> (shift - 1) & 0x1 != 0);
                    }
                    operand >> shift
                }
                // ASR
                2 => {
                    if change_status {
                        self.regs.set_c((operand as i32) >> (shift - 1) & 0x1 != 0)
                    };
                    ((operand as i32) >> shift) as u32
                }
                // ROR
                3 => {
                    if change_status {
                        self.regs.set_c(operand >> (shift - 1) & 0x1 != 0);
                    }
                    operand.rotate_right(shift)
                }
                _ => unreachable!(),
            }
        }
    }

    pub(self) fn add(&mut self, op1: u32, op2: u32, change_status: bool) -> u32 {
        let result = op1.overflowing_add(op2);
        if change_status {
            self.regs.set_c(result.1);
            self.regs.set_v((op1 as i32).overflowing_add(op2 as i32).1);
            self.regs.set_z(result.0 == 0);
            self.regs.set_n(result.0 & 0x8000_0000 != 0);
        }
        result.0
    }

    pub(self) fn adc(&mut self, op1: u32, op2: u32, change_status: bool) -> u32 {
        let result = op1.overflowing_add(op2);
        let result2 = result.0.overflowing_add(self.regs.get_c() as u32);
        if change_status {
            self.regs.set_c(result.1 || result2.1);
            self.regs.set_z(result2.0 == 0);
            self.regs.set_n(result2.0 & 0x8000_0000 != 0);
            self.regs
                .set_v((!(op1 ^ op2)) & (op1 ^ result2.0) & 0x8000_0000 != 0);
        }
        result2.0 as u32
    }

    pub(self) fn sub(&mut self, op1: u32, op2: u32, change_status: bool) -> u32 {
        let old_c = self.regs.get_c();
        self.regs.set_c(true);
        let result = self.adc(op1, !op2, change_status); // Simulate adding op1 + !op2 + 1
        if !change_status {
            self.regs.set_c(old_c)
        }
        result
    }

    pub(self) fn sbc(&mut self, op1: u32, op2: u32, change_status: bool) -> u32 {
        self.adc(op1, !op2, change_status)
    }

    pub(self) fn inc_mul_clocks(&mut self, op1: u32, signed: bool) {
        let mut mask = 0xFF_FF_FF_00;
        loop {
            self.internal();
            let value = op1 & mask;
            if mask == 0 || value == 0 || signed && value == mask {
                break;
            }
            mask <<= 8;
        }
    }
}
