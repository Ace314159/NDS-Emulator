use std::collections::HashMap;
use std::ops::Range;

use super::EngineType;

pub struct VRAM {
    cnts: [VRAMCNT; 9],
    banks: [Vec<u8>; 9],
    mappings: HashMap<u32, Vec<Mapping>>, // Maps from beginning of 16KB chunk to Mapping - TODO: Switch to using array
    mapping_ranges: [Range<u32>; 9], // Specifies Range that each bank encompasses
    // Functions
    lcdc_enabled: [bool; 9],
    engine_a_bg: [Option<Mapping>; 32],
    engine_a_obj: [Option<Mapping>; 16],
    engine_a_bg_ext_pal: [Option<Mapping>; 2],
    engine_a_obj_ext_pal: [Option<Mapping>; 1],
    engine_b_bg: [Option<Mapping>; 8],
    engine_b_obj: [Option<Mapping>; 8],
    engine_b_bg_ext_pal: [Option<Mapping>; 2],
    engine_b_obj_ext_pal: [Option<Mapping>; 1],
    arm7_wram: [Option<Bank>; 2],
}

impl VRAM {
    const BANKS_LEN: [usize; 9] = [128 * 0x400, 128 * 0x400, 128 * 0x400, 128 * 0x400,
        64 * 0x400, 16 * 0x400, 16 * 0x400, 32 * 0x400, 16 * 0x400];
    const MAPPING_LEN: usize = 16 * 0x400;

    const LCDC_START_ADDRESSES: [u32; 9] = [0x0680_0000, 0x0682_0000, 0x0684_0000, 0x0686_0000,
        0x0688_0000, 0x0689_0000, 0x0689_4000, 0x0689_8000, 0x068A_0000];
    const ENGINE_A_BG_START_ADDRESS: u32 = 0x0600_0000;
    const ENGINE_A_OBJ_START_ADDRESS: u32 = 0x0640_0000;
    const ENGINE_B_BG_START_ADDRESS: u32 = 0x0620_0000;
    const ENGINE_B_OBJ_START_ADDRESS: u32 = 0x0660_0000;
    
    const ENGINE_A_BG_VRAM_MASK: u32 = 4 * 128 * 0x400 - 1;
    const ENGINE_A_OBJ_VRAM_MASK: u32 = 2 * 128 * 0x400 - 1;
    const ENGINE_B_BG_VRAM_MASK: u32 = 1 * 128 * 0x400 - 1;
    const ENGINE_B_OBJ_VRAM_MASK: u32 = 1 * 128 * 0x400 - 1;

    // Can't cast in match :(
    const BANK_A: usize = Bank::A as usize;
    const BANK_B: usize = Bank::B as usize;
    const BANK_C: usize = Bank::C as usize;
    const BANK_D: usize = Bank::D as usize;
    const BANK_E: usize = Bank::E as usize;
    const BANK_F: usize = Bank::F as usize;
    const BANK_G: usize = Bank::G as usize;
    const BANK_H: usize = Bank::H as usize;
    const BANK_I: usize = Bank::I as usize;

    pub fn new() -> Self {
        VRAM {
            cnts: [VRAMCNT::new(0, 0); 9],
            banks: [
                vec![0; VRAM::BANKS_LEN[0]],
                vec![0; VRAM::BANKS_LEN[1]],
                vec![0; VRAM::BANKS_LEN[2]],
                vec![0; VRAM::BANKS_LEN[3]],
                vec![0; VRAM::BANKS_LEN[4]],
                vec![0; VRAM::BANKS_LEN[5]],
                vec![0; VRAM::BANKS_LEN[6]],
                vec![0; VRAM::BANKS_LEN[7]],
                vec![0; VRAM::BANKS_LEN[8]],
            ],
            mappings: HashMap::new(),
            mapping_ranges: [0..0, 0..0, 0..0, 0..0, 0..0, 0..0, 0..0, 0..0, 0..0],
            // Functions
            lcdc_enabled: [false; 9],
            engine_a_bg: [None; 32],
            engine_a_obj: [None; 16],
            engine_a_bg_ext_pal: [None; 2],
            engine_a_obj_ext_pal: [None; 1],
            engine_b_bg: [None; 8],
            engine_b_obj: [None; 8],
            engine_b_bg_ext_pal: [None; 2],
            engine_b_obj_ext_pal: [None; 1],
            arm7_wram: [None; 2],
        }
    }

    pub fn write_vram_cnt(&mut self, index: usize, value: u8) {
        let bank = Bank::from_index(index);
        let new_cnt = VRAMCNT::new(index, value);

        if self.cnts[index].enabled {
            for addr in self.mapping_ranges[index].clone().step_by(VRAM::MAPPING_LEN) {
                if let Some(mapping_vec) = self.mappings.get_mut(&addr) {
                    if let Some(i) = mapping_vec.iter().position(|&x| x.bank == bank) {
                        mapping_vec.swap_remove(i);
                    }
                }
            }
            match (index, self.cnts[index].mst) {
                (index, 0) => {
                    assert!(self.lcdc_enabled[index]);
                    self.lcdc_enabled[index] = false
                },
                (VRAM::BANK_A ..= VRAM::BANK_G, 1) => VRAM::remove_mapping(&self.mapping_ranges, index,
                    VRAM::ENGINE_A_BG_VRAM_MASK, &mut self.engine_a_bg),
                // TODO: Replace with match or syntax
                (VRAM::BANK_A ..= VRAM::BANK_B, 2) | (VRAM::BANK_E ..= VRAM::BANK_G, 2) =>
                    VRAM::remove_mapping(&self.mapping_ranges, index,
                    VRAM::ENGINE_A_OBJ_VRAM_MASK, &mut self.engine_a_obj),
                (VRAM::BANK_E, 4) => VRAM::remove_no_addr_mapping(&mut self.engine_a_bg_ext_pal,
                    0, 32 * 0x400),
                (VRAM::BANK_F ..= VRAM::BANK_G, 4) => VRAM::remove_no_addr_mapping(&mut self.engine_a_bg_ext_pal,
                    bank.get_ext_bg_pal_offset(self.cnts[index].mst), 16 * 0x400),
                (VRAM::BANK_F ..= VRAM::BANK_G, 5) => VRAM::remove_no_addr_mapping(&mut self.engine_a_obj_ext_pal,
                    0, 8 * 0x400),
                (VRAM::BANK_C, 4) | (VRAM::BANK_H ..= VRAM::BANK_I, 1) =>
                    VRAM::remove_mapping(&mut self.mapping_ranges, index,
                    VRAM::ENGINE_B_BG_VRAM_MASK, &mut self.engine_b_bg),
                (VRAM::BANK_D, 4) | (VRAM::BANK_I, 2) =>
                    VRAM::remove_mapping(&mut self.mapping_ranges, index,
                    VRAM::ENGINE_B_OBJ_VRAM_MASK, &mut self.engine_b_obj),
                (VRAM::BANK_H, 2) => VRAM::remove_no_addr_mapping(&mut self.engine_b_bg_ext_pal,
                    0, VRAM::BANKS_LEN[index]),
                (VRAM::BANK_I, 3) => VRAM::remove_no_addr_mapping(&mut self.engine_b_obj_ext_pal,
                    0, 8 * 0x400),
                (VRAM::BANK_C ..= VRAM::BANK_D, 2) => self.remove_arm7_wram_mapping(self.cnts[index].offset),
                (_index, 3 ..= 5) => warn!("Unimplemented VRAM Mapping {:?}: {}", bank, self.cnts[index].mst),
                _ => unreachable!(),
            }
        }

        self.cnts[index] = new_cnt;
        if !new_cnt.enabled { return }

        match (index, new_cnt.mst) {
            (index, 0) => {
                let start_addr = VRAM::LCDC_START_ADDRESSES[index];
                self.mapping_ranges[index] = start_addr..start_addr + VRAM::BANKS_LEN[index] as u32;
                for addr in self.mapping_ranges[index].clone().step_by(VRAM::MAPPING_LEN) {
                    self.mappings.entry(addr).or_insert_with(Vec::new)
                    .push(Mapping::new(bank, start_addr));
                }
                assert!(!self.lcdc_enabled[index]);
                self.lcdc_enabled[index] = true;
            },
            (VRAM::BANK_A ..= VRAM::BANK_G, 1) => VRAM::add_mapping(&mut self.mapping_ranges, &mut self.mappings,
                VRAM::ENGINE_A_BG_START_ADDRESS + bank.get_engine_a_offset(new_cnt.offset),
                bank, VRAM::ENGINE_A_BG_VRAM_MASK, &mut self.engine_a_bg),
            // TODO: Replace with match or syntax
            (VRAM::BANK_A ..= VRAM::BANK_B, 2) | (VRAM::BANK_E ..= VRAM::BANK_G, 2) =>
                VRAM::add_mapping(&mut self.mapping_ranges, &mut self.mappings,
                VRAM::ENGINE_A_OBJ_START_ADDRESS + bank.get_engine_a_offset(new_cnt.offset),
                bank, VRAM::ENGINE_A_OBJ_VRAM_MASK, &mut self.engine_a_obj),
            (VRAM::BANK_E, 4) => VRAM::add_no_addr_mapping(bank, &mut self.engine_a_bg_ext_pal,
                0, 32 * 0x400),
            (VRAM::BANK_F ..= VRAM::BANK_G, 4) => VRAM::add_no_addr_mapping(bank, &mut self.engine_a_bg_ext_pal,
                bank.get_ext_bg_pal_offset(self.cnts[index].mst), 16 * 0x400),
            (VRAM::BANK_F ..= VRAM::BANK_G, 5) => VRAM::add_no_addr_mapping(bank, &mut self.engine_a_obj_ext_pal,
                    0, 8 * 0x400),
            (VRAM::BANK_C, 4) | (VRAM::BANK_H, 1) =>
                VRAM::add_mapping(&mut self.mapping_ranges, &mut self.mappings, VRAM::ENGINE_B_BG_START_ADDRESS,
                bank, VRAM::ENGINE_B_BG_VRAM_MASK, &mut self.engine_b_bg),
            (VRAM::BANK_I, 1) =>
                VRAM::add_mapping(&mut self.mapping_ranges, &mut self.mappings,VRAM::ENGINE_B_BG_START_ADDRESS +
                0x8000, bank, VRAM::ENGINE_B_BG_VRAM_MASK, &mut self.engine_b_bg),
            (VRAM::BANK_D, 4) | (VRAM::BANK_I, 2) =>
                VRAM::add_mapping(&mut self.mapping_ranges, &mut self.mappings,VRAM::ENGINE_B_OBJ_START_ADDRESS,
                bank, VRAM::ENGINE_B_OBJ_VRAM_MASK, &mut self.engine_b_obj),
            (VRAM::BANK_H, 2) => VRAM::add_no_addr_mapping(bank, &mut self.engine_b_bg_ext_pal,
                0, VRAM::BANKS_LEN[index]),
            (VRAM::BANK_I, 3) => VRAM::add_no_addr_mapping(bank, &mut self.engine_b_obj_ext_pal,
                0, 8 * 0x400),
            (VRAM::BANK_C ..= VRAM::BANK_D, 2) => self.add_arm7_wram_mapping(bank, self.cnts[index].offset),
            (_index, 3 ..= 5) => warn!("Unimplemented VRAM Mapping {:?}: {}", bank, self.cnts[index].mst),
            _ => unreachable!(),
        }
    }

    pub fn get_lcdc_bank(&self, bank: u8) -> Option<&Vec<u8>> {
        if self.lcdc_enabled[bank as usize] { Some(&self.banks[bank as usize]) } else { None }
    }

    pub fn get_bg<E: EngineType>(&self, addr: usize) -> u8 {
        if E::is_a() {
            if let Some(mapping) = self.engine_a_bg[addr as usize / VRAM::MAPPING_LEN] {
                self.banks[mapping.bank as usize][mapping.offset as usize + addr % VRAM::MAPPING_LEN]
            } else { 0 }
        } else {
            if let Some(mapping) = self.engine_b_bg[addr as usize / VRAM::MAPPING_LEN] {
                self.banks[mapping.bank as usize][mapping.offset as usize + addr % VRAM::MAPPING_LEN]
            } else { 0 }
        }
    }

    pub fn get_obj<E: EngineType>(&self, addr: usize) -> u8 {
        if E::is_a() {
            if let Some(mapping) = self.engine_a_obj[addr as usize / VRAM::MAPPING_LEN] {
                self.banks[mapping.bank as usize][mapping.offset as usize + addr % VRAM::MAPPING_LEN]
            } else { 0 }
        } else {
            if let Some(mapping) = self.engine_b_obj[addr as usize / VRAM::MAPPING_LEN] {
                self.banks[mapping.bank as usize][mapping.offset as usize + addr % VRAM::MAPPING_LEN]
            } else { 0 }
        }
    }

    pub fn get_bg_ext_pal<E: EngineType>(&self, slot: usize, color_num: usize) -> u16 {
        let addr = self.calc_ext_pal_addr(slot, color_num);
        if E::is_a() {
            if let Some(mapping) = self.engine_a_bg_ext_pal[addr as usize / VRAM::MAPPING_LEN] {
                u16::from_le_bytes([
                    self.banks[mapping.bank as usize][mapping.offset as usize + addr % VRAM::MAPPING_LEN],
                    self.banks[mapping.bank as usize][mapping.offset as usize + addr % VRAM::MAPPING_LEN + 1],
                ])
            } else { 0 }
        } else {
            if let Some(mapping) = self.engine_b_bg_ext_pal[addr as usize / VRAM::MAPPING_LEN] {
                u16::from_le_bytes([
                    self.banks[mapping.bank as usize][mapping.offset as usize + addr % VRAM::MAPPING_LEN],
                    self.banks[mapping.bank as usize][mapping.offset as usize + addr % VRAM::MAPPING_LEN + 1],
                ])
            } else { 0 }
        }
    }

    pub fn get_obj_ext_pal<E: EngineType>(&self, color_num: usize) -> u16 {
        let addr = self.calc_ext_pal_addr(0, color_num);
        if E::is_a() {
            if let Some(mapping) = self.engine_a_obj_ext_pal[addr as usize / VRAM::MAPPING_LEN] {
                u16::from_le_bytes([
                    self.banks[mapping.bank as usize][mapping.offset as usize + addr % VRAM::MAPPING_LEN],
                    self.banks[mapping.bank as usize][mapping.offset as usize + addr % VRAM::MAPPING_LEN + 1],
                ])
            } else { 0 }
        } else {
            if let Some(mapping) = self.engine_b_obj_ext_pal[addr as usize / VRAM::MAPPING_LEN] {
                u16::from_le_bytes([
                    self.banks[mapping.bank as usize][mapping.offset as usize + addr % VRAM::MAPPING_LEN],
                    self.banks[mapping.bank as usize][mapping.offset as usize + addr % VRAM::MAPPING_LEN + 1],
                ])
            } else { 0 }
        }
    }

    pub fn get_arm7_wram(&self, addr: u32) -> Option<(&Vec<u8>, u32)> {
        let addr = addr - 0x0600_0000;
        let len = VRAM::BANKS_LEN[VRAM::BANK_C] as u32;
        // TODO: Figure out behavior
        assert!(addr < len * 2);
        let (bank, addr) = if addr < len {
            (self.arm7_wram[0], addr)
        } else {
            (self.arm7_wram[1], addr - len)
        };
        if let Some(bank) = bank { Some((&self.banks[bank as usize], addr)) } else { None }
    }

    pub fn get_arm7_wram_mut(&mut self, addr: u32) -> Option<(&mut Vec<u8>, u32)> {
        let addr = addr - 0x0600_0000;
        let len = VRAM::BANKS_LEN[VRAM::BANK_C] as u32;
        // TODO: Figure out behavior
        assert!(addr < len * 2);
        let (bank, addr) = if addr < len {
            (self.arm7_wram[0], addr)
        } else {
            (self.arm7_wram[1], addr - len)
        };
        if let Some(bank) = bank { Some((&mut self.banks[bank as usize], addr)) } else { None }
    }

    pub fn get_mem(&self, addr: u32) -> Option<(&Vec<u8>, u32)> {
        if let Some(mapping_vec) = self.mappings.get(&(addr & !(VRAM::MAPPING_LEN as u32 - 1))) {
            assert!(mapping_vec.len() < 2); // TODO: Support reading overlapping banks
            if let Some(mapping) = mapping_vec.first() {
                Some((&self.banks[mapping.bank as usize], addr - mapping.offset))
            } else { None }
        } else { None }
    }

    pub fn get_mem_mut(&mut self, addr: u32) -> Option<(&mut Vec<u8>, u32)> {
        if let Some(mapping_vec) = self.mappings.get(&(addr & !(VRAM::MAPPING_LEN as u32 - 1))) {
            assert!(mapping_vec.len() < 2); // TODO: Support writing overlapping banks
            if let Some(mapping) = mapping_vec.first() {
                Some((&mut self.banks[mapping.bank as usize], addr - mapping.offset))
            } else { None }
        } else { None }
    }

    pub fn calc_ext_pal_addr(&self, slot: usize, color_num: usize) -> usize {
        slot * 8 * 0x400 + color_num * 2
    }

    fn add_mapping(mapping_ranges: &mut [Range<u32>; 9], mappings: &mut HashMap<u32, Vec<Mapping>>,
        start_addr: u32, bank: Bank, mask: u32, arr: &mut [Option<Mapping>]) {
        mapping_ranges[bank as usize] = start_addr..start_addr + VRAM::BANKS_LEN[bank as usize] as u32;
        for (i, addr) in mapping_ranges[bank as usize].clone().step_by(VRAM::MAPPING_LEN).enumerate() {
            // TODO: Support overlapping banks
            mappings.entry(addr).or_insert_with(Vec::new).push(Mapping::new(bank, start_addr));
            arr[(addr & mask) as usize / VRAM::MAPPING_LEN] =
                Some(Mapping::new(bank, (VRAM::MAPPING_LEN * i) as u32));
        }
    }

    fn add_no_addr_mapping(bank: Bank, arr: &mut [Option<Mapping>], offset: usize, size: usize) {
        for (i, addr) in (0..size).step_by(VRAM::MAPPING_LEN).enumerate() {
            // TODO: Support overlapping banks
            assert!(arr[(addr + offset) / VRAM::MAPPING_LEN].is_none());
            arr[(addr + offset) / VRAM::MAPPING_LEN] = Some(Mapping::new(bank, (VRAM::MAPPING_LEN * i) as u32));
        }
    }

    fn remove_mapping(mapping_ranges: &[Range<u32>; 9], index: usize, mask: u32, arr: &mut [Option<Mapping>]) {
        for addr in mapping_ranges[index].clone().step_by(VRAM::MAPPING_LEN) {
            arr[(addr & mask) as usize / VRAM::MAPPING_LEN] = None;
        }
    }

    fn remove_no_addr_mapping(arr: &mut [Option<Mapping>], offset: usize, size: usize) {
        for addr in (0..size).step_by(VRAM::MAPPING_LEN) {
            arr[(addr + offset) / VRAM::MAPPING_LEN] = None;
        }
    }

    fn add_arm7_wram_mapping(&mut self, bank: Bank, offset: u8) {
        assert!(offset < 2);
        self.arm7_wram[offset as usize] = Some(bank);
    }

    fn remove_arm7_wram_mapping(&mut self, offset: u8) {
        assert!(offset < 2);
        self.arm7_wram[offset as usize] = None;
    }
}
// Corresponds to a bank and address offset into that bank
#[derive(Clone, Copy, Debug)]
struct Mapping {
    bank: Bank,
    offset: u32,
}

impl Mapping {
    pub fn new(bank: Bank, offset: u32) -> Mapping {
        Mapping {
            bank,
            offset,
        }
    }
}

#[derive(Clone, Copy)]
struct VRAMCNT {
    mst: u8,
    offset: u8,
    enabled: bool,
}

impl VRAMCNT {
    const MST_MASKS: [u8; 9] = [0x3, 0x3, 0x7, 0x7, 0x7, 0x7, 0x7, 0x3, 0x3];
    const OFS_MASKS: [u8; 9] = [0x3, 0x3, 0x3, 0x3, 0x0, 0x3, 0x3, 0x0, 0x0];

    pub fn new(index: usize, byte: u8) -> Self {
        VRAMCNT {
            mst: byte & VRAMCNT::MST_MASKS[index],
            offset: byte >> 3 & VRAMCNT::OFS_MASKS[index],
            enabled: byte >> 7 & 0x1 != 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq  )]
enum Bank {
    A = 0,
    B = 1,
    C = 2,
    D = 3,
    E = 4,
    F = 5,
    G = 6,
    H = 7,
    I = 8,
}

impl Bank {
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Bank::A,
            1 => Bank::B,
            2 => Bank::C,
            3 => Bank::D,
            4 => Bank::E,
            5 => Bank::F,
            6 => Bank::G,
            7 => Bank::H,
            8 => Bank::I,
            _ => unreachable!(),
        }
    }

    pub fn get_engine_a_offset(&self, offset: u8) -> u32 {
        let offset = offset as u32;
        match self {
            Bank::A | Bank::B | Bank::C | Bank::D => 0x2_0000 * offset,
            Bank::E => { assert_eq!(offset, 0); 0 },
            Bank::F | Bank::G => 0x4000 * (offset & 0x1) + 0x1_0000 * (offset >> 1 & 0x1),
            Bank::H | Bank::I => unreachable!(),
        }
    }

    pub fn get_ext_bg_pal_offset(&self, offset: u8) -> usize {
        assert!(offset < 2); // TODO: Figure out behavior
        offset as usize * 0x400 * 16
    }
}
