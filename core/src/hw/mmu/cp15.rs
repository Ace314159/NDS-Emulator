use bitflags::*;

pub struct CP15 {
    control: Control,
}

impl CP15 {
    pub fn new() -> Self {
        CP15 {
            control: Control::new(),
        }
    }

    pub fn read(&self, n: u32, m: u32, p: u32) -> u32 {
        info!("Reading from C{}, C{}, {}", n, m, p);
        todo!()
    }

    pub fn write(&mut self, n: u32, m: u32, p: u32, value: u32) {
        info!("Writing 0b{:b} to C{}, C{}, {}", value, n, m, p);
        match n {
            1 => self.write_control_reg(m, p, value),
            _ => todo!(),
        }
    }

    fn write_control_reg(&mut self, m: u32, p: u32, value: u32) {
        if m != 0 || p != 0 { warn!("m and p are not 0 for CP15 Control Register Write: {} {}", m, p); return }
        self.control.bits = value & Control::MASK | Control::ALWAYS_SET;
        assert!(self.control.contains(Control::EXCEPTION_VECTORS)); // TODO: Implement Exception vectors at 0
    }
}

bitflags! {
    struct Control: u32 {
        const ITCM_WRITE_ONLY = 1 << 19;
        const ITCM_ENABLE = 1 << 18;
        const DTCM_WRITE_ONLY = 1 << 17;
        const DTCM_ENABLE = 1 << 16;
        const PRE_ARMV5 = 1 << 15;
        const CACHE_REPLACEMENT = 1 << 14;
        const EXCEPTION_VECTORS = 1 << 13;
        const INSTR_CACHE_ENABLE = 1 << 12;
        const BRANCH_PREDICTION = 1 << 11;
        const BIG_ENDIAN = 1 << 7;
        const LATE_ABORT = 1 << 6;
        const ADDRESS_FAULTS_32 = 1 << 5;
        const EXCEPTION_HANDLING_32 = 1 << 4;
        const WRITE_BUFFER_ENABLE = 1 << 3;
        const DATA_UNIFIED_CACHE_ENABLE = 1 << 2;
        const ALIGNMENT_FAULT_CHECK = 1 << 1;
        const PU_ENABLE = 1 << 0;
    }
}

impl Control {
    const MASK: u32 = (1 << 19) | (1 << 18) | (1 << 17) | (1 << 16) | (1 << 15) | (1 << 14) | (1 << 13) | (1 << 12) |
        (1 << 7) | (1 << 2) | (1 << 0);
    const ALWAYS_SET: u32 = (1 << 6) | (1 << 5) | (1 << 4) | (1 << 3);

    pub fn new() -> Self {
        Control::from_bits(Control::ALWAYS_SET).unwrap()
    }
}
