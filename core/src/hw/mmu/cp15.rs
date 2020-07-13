use bitflags::*;

use super::HW;

pub struct CP15 {
    control: Control,
    interrupt_base: u32,
    itcm_control: TCMControl,
    dtcm_control: TCMControl,
}

impl CP15 {
    pub fn new() -> Self {
        CP15 {
            control: Control::new(),
            interrupt_base: 0xFFF_0000,
            itcm_control: TCMControl::new(0, HW::ITCM_SIZE as u32),
            dtcm_control: TCMControl::new(0x0080_3000, HW::DTCM_SIZE as u32),
        }
    }

    pub fn read(&self, n: u32, m: u32, p: u32) -> u32 {
        info!("Reading from C{}, C{}, {}", n, m, p);
        match n {
            1 => self.read_control_reg(m, p),
            _ => todo!(),
        }
    }

    pub fn write(&mut self, n: u32, m: u32, p: u32, value: u32) {
        info!("Writing 0b{:b} to C{}, C{}, {}", value, n, m, p);
        match n {
            1 => self.write_control_reg(m, p, value),
            2 => warn!("Ignoring MMU Translation Table Base Write: C{}, C{}, {}: 0x{:X}", n, m, p, value),
            3 => warn!("Ignoring MMU Domain Access Control Write: C{}, C{}, {}: 0x{:X}", n, m, p, value),
            5 => warn!("Ignoring MMU Domain Fault Status Write: C{}, C{}, {}: 0x{:X}", n, m, p, value),
            6 => self.write_pu_regions(m, p, value),
            7 => self.write_cache_command(m, p, value),
            8 => warn!("Ignoring MMU TLB Control Write: C{}, C{}, {}: 0x{:X}", n, m, p, value),
            9 => self.write_cache(m, p, value),
            10 => warn!("Ignoring MMU TLB Lockdown Write: C{}, C{}, {}: 0x{:X}", n, m, p, value),
            _ => todo!(),
        }
    }

    pub fn addr_in_itcm(&self, addr: u32) -> bool {
        addr < self.itcm_control.virtual_size
    }

    pub fn addr_in_dtcm(&self, addr: u32) -> bool {
        (self.dtcm_control.base..self.dtcm_control.base + self.dtcm_control.virtual_size).contains(&addr)
    }

    fn read_control_reg(&self, m: u32, p: u32) -> u32 {
        if m != 0 || p != 0 { warn!("m and p are not 0 for CP15 Control Register Read: {} {}", m, p); return 0 }
        self.control.bits
    }

    fn write_control_reg(&mut self, m: u32, p: u32, value: u32) {
        if m != 0 || p != 0 { warn!("m and p are not 0 for CP15 Control Register Write: {} {}", m, p); return }
        self.control.bits = value & Control::MASK | Control::ALWAYS_SET;
        self.interrupt_base = if self.control.contains(Control::INTERRUPT_BASE) { 0xFFFF_0000 } else { 0x0000_0000 };
    }

    fn write_pu_regions(&mut self, m: u32, p: u32, value: u32) {
        match p {
            0 => warn!("PU Data/Unified Region {}: 0x{:X}", m, value),
            1 => warn!("PU Instruction Region {}: 0x{:X}", m, value),
            _ => warn!("Ignoring MMU Fault Address Write : C{}, C{}, {}: 0x{:X}", 6, m, p, value),
        }
    }

    fn write_cache_command(&mut self, m: u32, p: u32, value: u32) {
        match (m, p) {
            (5, 0) if value == 0 => warn!("Invalidate Entire Instruction Cache"), // TODO: Invalidate Entire Instruction Cache
            (6, 0) if value == 0 => warn!("Invalidate Entire Data Cache"), // TODO: Invalidate Entire Data Cache
            (10, 4) if value == 0 => warn!("Drain Write Buffer"), // TODO: Drain Write Buffer
            _ => todo!(),
        }
    }

    fn write_cache(&mut self, m: u32, p: u32, value: u32) {
        match (m, p) {
            (0, 0) => warn!("Data Cache Lockdown: 0x{:X}", value), // TODO: Data Cache Lockdown
            (0, 1) => warn!("Instruction Cache Lockdown: 0x{:X}", value), // TODO: Instruction Cache Lockdown
            (1, 0) => self.dtcm_control.write(value),
            (1, 1) => { self.itcm_control.write(value); assert_eq!(self.itcm_control.base, 0) },
            _ => todo!(),
        }
    }
}

struct TCMControl {
    pub base: u32,
    pub virtual_size: u32,
}

impl TCMControl {
    pub fn new(base: u32, virtual_size: u32) -> Self {
        TCMControl {
            base,
            virtual_size,
        }
    }

    /*pub fn read(&self) -> u32 {
        self.base << 12 | self.virtual_size << 1
    }*/

    pub fn write(&mut self, value: u32) {
        self.base = value & !0xFFF;
        let size_shift = value >> 1 & 0x1F;
        assert!((3..=23).contains(&size_shift));
        self.virtual_size = 0x200 << size_shift;
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
        const INTERRUPT_BASE = 1 << 13;
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

impl CP15 {
    pub fn interrupt_base(&self) -> u32 { self.interrupt_base }
}
