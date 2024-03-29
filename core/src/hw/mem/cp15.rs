use _core::ops::Range;
use bitflags::*;

use super::HW;

pub struct CP15 {
    control: Control,
    interrupt_base: u32,
    itcm_control: TCMControl,
    dtcm_control: TCMControl,
    pub arm9_halted: bool,
    // AP Regions
    ap_data_region: u32,
    ap_instr_region: u32,
    ext_ap_data_region: u32,
    ext_ap_instr_region: u32,
    // PU Regions
    pu_data_regions: [u32; 8],
    pu_instr_regions: [u32; 8],
}

impl CP15 {
    pub fn new() -> Self {
        CP15 {
            control: Control::new(),
            interrupt_base: 0xFFFF_0000,
            itcm_control: TCMControl::new(0, HW::ITCM_SIZE as u32),
            dtcm_control: TCMControl::new(0x0080_3000, HW::DTCM_SIZE as u32),
            arm9_halted: false,
            // AP Regions
            ap_data_region: 0,
            ap_instr_region: 0,
            ext_ap_data_region: 0,
            ext_ap_instr_region: 0,
            // PU Regions
            pu_data_regions: [0; 8],
            pu_instr_regions: [0; 8],
        }
    }

    pub fn read(&self, n: u32, m: u32, p: u32) -> u32 {
        info!("Reading from C{}, C{}, {}", n, m, p);
        match n {
            0 if (m, p) == (0, 1) => 0x0F0D2112, // Cache Type Register
            1 => self.read_control_reg(m, p),
            5 => self.read_ap_regions(m, p),
            6 => self.read_pu_regions(m, p),
            9 => self.read_cache_control(m, p),
            _ => todo!(),
        }
    }

    pub fn write(&mut self, n: u32, m: u32, p: u32, value: u32) {
        info!("Writing 0b{:b} to C{}, C{}, {}", value, n, m, p);
        match n {
            1 => self.write_control_reg(m, p, value),
            2 => self.write_cachability(m, p, value),
            3 => self.write_cache_write_bufferability(m, p, value),
            5 => self.write_ap_regions(m, p, value),
            6 => self.write_pu_regions(m, p, value),
            7 => self.write_cache_command(m, p, value),
            9 => self.write_cache_control(m, p, value),
            _ => todo!(),
        }
    }

    pub fn itcm_range(&self) -> Range<u32> {
        0..self.itcm_control.virtual_size
    }

    pub fn dtcm_range(&self) -> Range<u32> {
        self.dtcm_control.base..self.dtcm_control.base + self.dtcm_control.virtual_size
    }

    fn read_control_reg(&self, m: u32, p: u32) -> u32 {
        if m != 0 || p != 0 {
            warn!(
                "m and p are not 0 for CP15 Control Register Read: {} {}",
                m, p
            );
            return 0;
        }
        self.control.bits
    }

    fn read_ap_regions(&self, m: u32, p: u32) -> u32 {
        match (m, p) {
            (0, 0) => self.ap_data_region,
            (0, 1) => self.ap_instr_region,
            (0, 2) => self.ext_ap_data_region,
            (0, 3) => self.ext_ap_instr_region,
            _ => todo!(),
        }
    }

    fn read_pu_regions(&self, m: u32, p: u32) -> u32 {
        match (m, p) {
            (region, 0) => self.pu_data_regions[region as usize],
            (region, 1) => self.pu_instr_regions[region as usize],
            _ => todo!(),
        }
    }

    fn write_control_reg(&mut self, m: u32, p: u32, value: u32) {
        if m != 0 || p != 0 {
            warn!(
                "m and p are not 0 for CP15 Control Register Write: {} {}",
                m, p
            );
            return;
        }
        info!("Writing to CP15 Control Register 0x{:X}", value);
        self.control.bits = value & Control::MASK | Control::ALWAYS_SET;
        self.interrupt_base = if self.control.contains(Control::INTERRUPT_BASE) {
            0xFFFF_0000
        } else {
            0x0000_0000
        };
    }

    fn write_cachability(&mut self, m: u32, p: u32, value: u32) {
        match (m, p) {
            (0, 0) => warn!("Cachability Bits for Data/Unified Region: 0x{:X}", value),
            (0, 1) => warn!("Cachability Bits for Instruction Region: 0x{:X}", value),
            _ => todo!(),
        }
    }

    fn write_cache_write_bufferability(&mut self, m: u32, p: u32, value: u32) {
        if m != 0 || p != 0 {
            warn!(
                "m and p are not 0 for CP15 Cache write Bufferability: {} {}",
                m, p
            );
            return;
        }
        info!("Cache Write Bufferability: 0x{:X}", value);
    }

    fn write_ap_regions(&mut self, m: u32, p: u32, value: u32) {
        match (m, p) {
            (0, 0) => self.ap_data_region = value & 0xFFFF,
            (0, 1) => self.ap_instr_region = value & 0xFFFF,
            (0, 2) => self.ext_ap_data_region = value,
            (0, 3) => self.ext_ap_instr_region = value,
            _ => todo!(),
        }
    }

    fn write_pu_regions(&mut self, m: u32, p: u32, value: u32) {
        match (m, p) {
            (region, 0) => self.pu_data_regions[region as usize] = value & !(0x3F << 6),
            (region, 1) => self.pu_instr_regions[region as usize] = value & !(0x3F << 6),
            _ => todo!(),
        }
    }

    fn write_cache_command(&mut self, m: u32, p: u32, value: u32) {
        match (m, p) {
            (0, 4) if value == 0 => self.arm9_halted = true,
            (5, 0) if value == 0 => info!("Invalidate Entire Instruction Cache"), // TODO: Invalidate Entire Instruction Cache
            (5, 1) => info!("Invalidate Instruction Cache Line 0x{:X}", value), // TODO: Invalidate Instruction Cache Line
            (6, 0) if value == 0 => info!("Invalidate Entire Data Cache"), // TODO: Invalidate Entire Data Cache
            (6, 1) => info!("Invalidate Data Cache Line 0x{:X}", value), // TODO: Invalidate Data Cache Line
            (10, 1) => info!("Clean Data Cache Line 0x{:X}", value), // TODO: Clean Data Cache Line
            (10, 2) => info!("Clean Data Cache Line Index 0x{:X}", value), // TODO: Clean Data Cache Line
            (10, 4) if value == 0 => info!("Drain Write Buffer"), // TODO: Drain Write Buffer
            (14, 1) => info!("Clean and Invalidate Data Cache Line 0x{:X}", value), // TODO: Clean and Invalidate Data Cache Line
            (14, 2) => info!("Clean and Invalidate Data Cache Index 0x{:X}", value), // TODO: Clean and Invalidate Data Cache Line
            _ => todo!(),
        }
    }

    fn read_cache_control(&self, m: u32, p: u32) -> u32 {
        match (m, p) {
            (1, 0) => self.dtcm_control.read(),
            (1, 1) => self.itcm_control.read(),
            _ => todo!(),
        }
    }

    fn write_cache_control(&mut self, m: u32, p: u32, value: u32) {
        match (m, p) {
            (0, 0) => warn!("Data Cache Lockdown: 0x{:X}", value), // TODO: Data Cache Lockdown
            (0, 1) => warn!("Instruction Cache Lockdown: 0x{:X}", value), // TODO: Instruction Cache Lockdown
            (1, 0) => self.dtcm_control.write(value),
            (1, 1) => {
                self.itcm_control.write(value);
                assert_eq!(self.itcm_control.base, 0)
            }
            _ => todo!(),
        }
    }
}

struct TCMControl {
    pub base: u32,
    pub virtual_size: u32,
    virtual_size_shift: u32,
}

impl TCMControl {
    pub fn new(base: u32, virtual_size: u32) -> Self {
        let mut v_size_copy = virtual_size;
        let mut shift = 0;
        while v_size_copy != 0x200 {
            shift += 1;
            v_size_copy >>= 1;
            assert!(v_size_copy >= 0x200);
        }
        TCMControl {
            base,
            virtual_size,
            virtual_size_shift: shift,
        }
    }

    pub fn read(&self) -> u32 {
        self.base | self.virtual_size_shift << 1
    }

    pub fn write(&mut self, value: u32) {
        self.base = value & !0xFFF;
        self.virtual_size_shift = value >> 1 & 0x1F;
        assert!((3..=23).contains(&self.virtual_size_shift));
        self.virtual_size = 0x200 << self.virtual_size_shift;
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
    const MASK: u32 = (1 << 19)
        | (1 << 18)
        | (1 << 17)
        | (1 << 16)
        | (1 << 15)
        | (1 << 14)
        | (1 << 13)
        | (1 << 12)
        | (1 << 7)
        | (1 << 2)
        | (1 << 0);
    const ALWAYS_SET: u32 = (1 << 6) | (1 << 5) | (1 << 4) | (1 << 3);

    pub fn new() -> Self {
        Control::from_bits(0x52078).unwrap()
    }
}

impl CP15 {
    pub fn interrupt_base(&self) -> u32 {
        self.interrupt_base
    }
}
