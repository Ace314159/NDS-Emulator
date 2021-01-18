use bitflags::*;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Mode {
    USR = 0b10000,
    FIQ = 0b10001,
    IRQ = 0b10010,
    SVC = 0b10011,
    ABT = 0b10111,
    SYS = 0b11111,
    UND = 0b11011,
}

bitflags! {
    struct StatusReg: u32 {
        const N =  0x80000000;
        const Z =  0x40000000;
        const C =  0x20000000;
        const V =  0x10000000;
        const Q =  0x08000000;
        const F =  0x00000040;
        const I =  0x00000080;
        const T =  0x00000020;
        const M4 = 0x00000010;
        const M3 = 0x00000008;
        const M2 = 0x00000004;
        const M1 = 0x00000002;
        const M0 = 0x00000001;
    }
}

impl StatusReg {
    pub fn reset() -> StatusReg {
        StatusReg::from_bits(Mode::SYS as u32).unwrap()
    }

    pub fn get_mode(&self) -> Mode {
        match Some(self.bits() & 0x1F) {
            Some(m) if m == Mode::USR as u32 => Mode::USR,
            Some(m) if m == Mode::FIQ as u32 => Mode::FIQ,
            Some(m) if m == Mode::IRQ as u32 => Mode::IRQ,
            Some(m) if m == Mode::SVC as u32 => Mode::SVC,
            Some(m) if m == Mode::ABT as u32 => Mode::ABT,
            Some(m) if m == Mode::SYS as u32 => Mode::SYS,
            Some(m) if m == Mode::UND as u32 => Mode::UND,
            _ => panic!("Invalid Mode"),
        }
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.bits = (self.bits() & !0x1F) | mode as u32;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RegValues {
    regs: [u32; 16],
    usr: [u32; 2], // R13 and R14
    svc: [u32; 2], // R13 and R14
    irq: [u32; 2], // R13 and R14
    cpsr: StatusReg,
    spsr: [StatusReg; 2], // SVC and IRQ
}

impl RegValues {
    pub fn new() -> RegValues {
        RegValues {
            regs: [0; 16],
            usr: [0; 2], // R13 and R14
            svc: [0; 2], // R13 and R14
            irq: [0; 2], // R13 and R14
            cpsr: StatusReg::reset(),
            spsr: [StatusReg::reset(); 2], // SVC and IRQ
        }
    }

    pub fn no_bios(pc: u32) -> RegValues {
        let mut reg_values = RegValues::new();
        reg_values.regs[12] = pc;
        reg_values.regs[13] = 0x03003FC0;
        reg_values.regs[15] = pc;
        reg_values.irq[0] = 0x03003F80; // R13
        reg_values.svc[0] = 0x03002F7C; // R13
        reg_values.svc[1] = pc; // R14
        reg_values.cpsr.bits = 0xD3;
        reg_values
    }

    pub fn change_mode(&mut self, mode: Mode) {
        self.save_banked();
        let cpsr = self.cpsr();
        self.cpsr.set_mode(mode);
        self.load_banked(mode);
        *self.spsr_mut() = cpsr;
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.save_banked();
        self.cpsr.set_mode(mode);
        self.load_banked(mode);
    }

    pub fn restore_cpsr(&mut self) {
        self.save_banked();
        self.cpsr.bits = self.spsr();
        self.load_banked(self.cpsr.get_mode());
    }

    pub fn save_banked(&mut self) {
        match self.cpsr.get_mode() {
            Mode::USR | Mode::SYS  => self.usr.copy_from_slice(&self.regs[13..15]),
            Mode::SVC => self.svc.copy_from_slice(&self.regs[13..15]),
            Mode::IRQ => self.irq.copy_from_slice(&self.regs[13..15]),
            _ => unreachable!(), // Unused modes (hopefully)
        }
    }

    pub fn load_banked(&mut self, mode: Mode) {
        assert_eq!(self.cpsr.get_mode(), mode);
        let banked = match mode {
            Mode::USR | Mode::SYS => &self.usr,
            Mode::SVC => &self.svc,
            Mode::IRQ => &self.irq,
            _ => unreachable!(), // Unused modes (hopefully)
        };
        self.regs[13..15].copy_from_slice(banked);
    }

    pub fn spsr(&self) -> u32 {
        match self.cpsr.get_mode() {
            Mode::SVC => self.spsr[0].bits,
            Mode::IRQ => self.spsr[1].bits,
            Mode::FIQ | Mode::ABT | Mode::UND => unreachable!(), // Unused modes (hopefully)
            _ => self.cpsr.bits,
        }
    }

    pub fn spsr_mut(&mut self) -> &mut u32 {
        match self.cpsr.get_mode() {
            Mode::SVC => &mut self.spsr[0].bits,
            Mode::IRQ => &mut self.spsr[1].bits,
            Mode::FIQ | Mode::ABT | Mode::UND => unreachable!(), // Unused modes (hopefully)
            _ => &mut self.cpsr.bits,
        }
    }

    pub fn cpsr(&self) -> u32 {
        self.cpsr.bits
    }

    pub fn cpsr_mut(&mut self) -> &mut u32 {
        &mut self.cpsr.bits
    }

    pub fn sp(&self) -> u32{
        self.regs[13]
    }

    pub fn set_sp(&mut self, value: u32) {
        self.regs[13] = value;
    }

    pub fn lr(&self) -> u32 {
        self.regs[14]
    }

    pub fn set_lr(&mut self, value: u32) {
        self.regs[14] = value;
    }

    pub fn _get_n(&self) -> bool { self.cpsr.contains(StatusReg::N) }
    pub fn _get_z(&self) -> bool { self.cpsr.contains(StatusReg::Z) }
    pub fn get_c(&self) -> bool { self.cpsr.contains(StatusReg::C) }
    pub fn _get_v(&self) -> bool { self.cpsr.contains(StatusReg::V) }
    pub fn _get_q(&self) -> bool { self.cpsr.contains(StatusReg::Q) }
    pub fn get_i(&self) -> bool { self.cpsr.contains(StatusReg::I) }
    pub fn _get_f(&self) -> bool { self.cpsr.contains(StatusReg::F) }
    pub fn get_flags(&self) -> u32 { self.cpsr.bits >> 24 }
    pub fn get_t(&self) -> bool { self.cpsr.contains(StatusReg::T) }
    pub fn get_mode(&self) -> Mode { self.cpsr.get_mode() }
    pub fn set_n(&mut self, value: bool) { self.cpsr.set(StatusReg::N, value) }
    pub fn set_z(&mut self, value: bool) { self.cpsr.set(StatusReg::Z, value) }
    pub fn set_c(&mut self, value: bool) { self.cpsr.set(StatusReg::C, value) }
    pub fn set_v(&mut self, value: bool) { self.cpsr.set(StatusReg::V, value) }
    pub fn set_q(&mut self, value: bool) { self.cpsr.set(StatusReg::Q, value) }
    pub fn set_i(&mut self, value: bool) { self.cpsr.set(StatusReg::I, value) }
    pub fn _set_f(&mut self, value: bool) { self.cpsr.set(StatusReg::F, value) }
    pub fn set_t(&mut self, value: bool) { self.cpsr.set(StatusReg::T, value) }
    //fn set_mode(&mut self, mode: Mode) { self.cpsr.set_mode(mode) }
}

impl std::ops::Index<u32> for RegValues {
    type Output = u32;

    fn index(&self, index: u32) -> &Self::Output {
        &self.regs[index as usize]
    }
}

impl std::ops::IndexMut<u32> for RegValues {
    fn index_mut(&mut self, index: u32) -> &mut Self::Output {
        &mut self.regs[index as usize]
    }
}
