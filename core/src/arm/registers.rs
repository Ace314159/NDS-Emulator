use bitfield::bitfield;

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

bitfield! {
    #[derive(Debug, PartialEq, Clone, Copy)]
    struct StatusRegBits: u32 {
        n: bool @ 31,
        z: bool @ 30,
        c: bool @ 29,
        v: bool @ 28,
        q: bool @ 27,
        _: _ @ 8..=26,
        i: bool @ 7,
        f: bool @ 6,
        t: bool @ 5,
        mode: u8 @ 0..=4,
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
struct StatusReg {
    pub bits: StatusRegBits,
    pub mode: Mode,
}

impl StatusReg {
    pub fn reset() -> StatusReg {
        StatusReg {
            bits: StatusRegBits::new(),
            mode: Mode::SYS,
        }
    }

    pub fn get_mode(&self) -> Mode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.bits.set_mode(mode as u8);
        self.mode = mode;
    }

    pub fn update_mode(&mut self) {
        self.mode = match self.bits.mode() {
            bits if bits == Mode::USR as u8 => Mode::USR,
            bits if bits == Mode::FIQ as u8 => Mode::FIQ,
            bits if bits == Mode::IRQ as u8 => Mode::IRQ,
            bits if bits == Mode::SVC as u8 => Mode::SVC,
            bits if bits == Mode::ABT as u8 => Mode::ABT,
            bits if bits == Mode::SYS as u8 => Mode::SYS,
            bits if bits == Mode::UND as u8 => Mode::UND,
            _ => panic!("Invalid Mode"),
        };
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RegValues {
    regs: [u32; 16],
    svc: [u32; 2], // R13 and R14
    und: [u32; 2], // R13 and R14
    irq: [u32; 2], // R13 and R14
    fiq: [u32; 7], // R8-R14
    cpsr: StatusReg,
    spsr: [StatusReg; 4], // SVC, IRQ, IRQ
}

impl RegValues {
    pub fn new<const IS_ARM9: bool>() -> RegValues {
        let mut regs = RegValues {
            regs: [0; 16],
            svc: [0; 2], // R13 and R14
            und: [0; 2], // R13 and R14
            irq: [0; 2], // R13 and R14
            fiq: [0; 7], // R8-R14
            cpsr: StatusReg::reset(),
            spsr: [StatusReg::reset(); 4], // SVC, UND, IRQ, FIQ
        };
        regs[15] = if IS_ARM9 { 0xFFFF_0000 } else { 0x0 };
        regs
    }

    pub fn direct_boot<const IS_ARM9: bool>(pc: u32) -> RegValues {
        let mut reg_values = RegValues::new::<IS_ARM9>();
        // regs contains svc banked
        // svc contains usr banked
        // TODO: Figure out actual values
        reg_values.regs[12] = pc; // R12
        reg_values.regs[1] = pc; // R14
        reg_values.cpsr.bits.0 = 0xD3;
        reg_values.cpsr.update_mode();
        if IS_ARM9 {
            reg_values.svc[0] = 0x03003FC0; // R13
            reg_values.irq[0] = 0x03003F80; // R13
            reg_values.regs[13] = 0x03002F7C; // R13
        } else {
            reg_values.svc[0] = 0x0380FFC0; // R13
            reg_values.irq[0] = 0x0380FF80; // R13
            reg_values.regs[13] = 0x0380FD80; // R13
        };
        assert_eq!(reg_values.get_mode(), Mode::SVC);
        reg_values.regs[15] = pc;
        reg_values
    }

    pub fn change_mode(&mut self, mode: Mode) {
        self.save_banked();
        let cpsr = self.cpsr();
        self.cpsr.set_mode(mode);
        self.load_banked(mode);
        *self.spsr_mut() = cpsr;
        self.update_spsr_mode();
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.save_banked();
        self.cpsr.set_mode(mode);
        self.load_banked(mode);
    }

    pub fn restore_cpsr(&mut self) {
        self.save_banked();
        self.cpsr.bits.0 = self.spsr();
        self.cpsr.update_mode();
        self.load_banked(self.cpsr.get_mode());
    }

    pub fn save_banked(&mut self) {
        let banked: &mut [u32] = match self.cpsr.get_mode() {
            Mode::USR | Mode::SYS => return,
            Mode::SVC => &mut self.svc,
            Mode::UND => &mut self.und,
            Mode::IRQ => &mut self.irq,
            Mode::FIQ => &mut self.fiq,
            Mode::ABT => unreachable!(), // Unused modes (hopefully)
        };
        let start = 15 - banked.len();
        banked.swap_with_slice(&mut self.regs[start..15]);
    }

    pub fn load_banked(&mut self, mode: Mode) {
        assert_eq!(self.cpsr.get_mode(), mode);
        let banked: &mut [u32] = match mode {
            Mode::USR | Mode::SYS => return,
            Mode::SVC => &mut self.svc,
            Mode::UND => &mut self.und,
            Mode::IRQ => &mut self.irq,
            Mode::FIQ => &mut self.fiq,
            Mode::ABT => unreachable!(), // Unused modes (hopefully)
        };
        let start = 15 - banked.len();
        self.regs[start..15].swap_with_slice(banked);
    }

    pub fn spsr(&self) -> u32 {
        match self.cpsr.get_mode() {
            Mode::SVC => self.spsr[0].bits.0,
            Mode::UND => self.spsr[1].bits.0,
            Mode::IRQ => self.spsr[2].bits.0,
            Mode::FIQ => self.spsr[3].bits.0,
            Mode::ABT => unreachable!(), // Unused modes (hopefully)
            _ => self.cpsr.bits.0,
        }
    }

    // Make sure to update the mode after using
    pub fn spsr_mut(&mut self) -> &mut u32 {
        match self.cpsr.get_mode() {
            Mode::SVC => &mut self.spsr[0].bits.0,
            Mode::UND => &mut self.spsr[1].bits.0,
            Mode::IRQ => &mut self.spsr[2].bits.0,
            Mode::FIQ => &mut self.spsr[3].bits.0,
            Mode::ABT => unreachable!(), // Unused modes (hopefully)
            _ => &mut self.cpsr.bits.0,
        }
    }

    pub fn update_spsr_mode(&mut self) {
        match self.cpsr.get_mode() {
            Mode::SVC => &mut self.spsr[0].update_mode(),
            Mode::UND => &mut self.spsr[1].update_mode(),
            Mode::IRQ => &mut self.spsr[2].update_mode(),
            Mode::FIQ => &mut self.spsr[3].update_mode(),
            Mode::ABT => unreachable!(), // Unused modes (hopefully)
            _ => &mut self.cpsr.update_mode(),
        };
    }

    pub fn cpsr(&self) -> u32 {
        self.cpsr.bits.0
    }

    // Make sure to update the mode after using
    pub fn cpsr_mut(&mut self) -> &mut u32 {
        &mut self.cpsr.bits.0
    }

    pub fn update_cpsr_mode(&mut self) {
        self.cpsr.update_mode();
    }

    pub fn sp(&self) -> u32 {
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

    pub fn _get_n(&self) -> bool {
        self.cpsr.bits.n()
    }
    pub fn _get_z(&self) -> bool {
        self.cpsr.bits.z()
    }
    pub fn get_c(&self) -> bool {
        self.cpsr.bits.c()
    }
    pub fn _get_v(&self) -> bool {
        self.cpsr.bits.v()
    }
    pub fn _get_q(&self) -> bool {
        self.cpsr.bits.q()
    }
    pub fn get_i(&self) -> bool {
        self.cpsr.bits.i()
    }
    pub fn _get_f(&self) -> bool {
        self.cpsr.bits.f()
    }
    pub fn get_flags(&self) -> u32 {
        self.cpsr.bits.0 >> 24
    }
    pub fn get_t(&self) -> bool {
        self.cpsr.bits.t()
    }
    pub fn get_mode(&self) -> Mode {
        self.cpsr.get_mode()
    }
    pub fn set_n(&mut self, value: bool) {
        self.cpsr.bits.set_n(value);
    }
    pub fn set_z(&mut self, value: bool) {
        self.cpsr.bits.set_z(value);
    }
    pub fn set_c(&mut self, value: bool) {
        self.cpsr.bits.set_c(value);
    }
    pub fn set_v(&mut self, value: bool) {
        self.cpsr.bits.set_v(value);
    }
    pub fn set_q(&mut self, value: bool) {
        self.cpsr.bits.set_q(value);
    }
    pub fn set_i(&mut self, value: bool) {
        self.cpsr.bits.set_i(value);
    }
    pub fn _set_f(&mut self, value: bool) {
        self.cpsr.bits.set_f(value);
    }
    pub fn set_t(&mut self, value: bool) {
        self.cpsr.bits.set_t(value);
    }
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
