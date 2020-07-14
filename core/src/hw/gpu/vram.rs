use std::collections::HashMap;
use std::ops::Range;

pub struct VRAM {
    cnts: [VRAMCNT; 9],
    banks: [Vec<u8>; 9],
    mappings: HashMap<u32, Mapping>, // Maps from beginning of 16KB chunk to Mapping - TODO: Switch to using array
    mapping_ranges: [Range<u32>; 9], // Specifies Range that each bank encompasses
    // Functions
    lcdc_enabled: [bool; 9],
    engine_a_bg: [Option<Mapping>; 32],
}

impl VRAM {
    const BANKS_LEN: [usize; 9] = [128 * 0x400, 128 * 0x400, 128 * 0x400, 128 * 0x400,
        64 * 0x400, 16 * 0x400, 16 * 0x400, 32 * 0x400, 16 * 0x400];
    const MAPPING_LEN: usize = 16 * 0x400;

    const LCDC_ADDRESSES: [u32; 9] = [0x0680_0000, 0x0682_0000, 0x0684_0000, 0x0686_0000,
        0x0688_0000, 0x0689_0000, 0x0689_4000, 0x0689_8000, 0x068A_0000];
    const ENGINE_A_BG_START_ADDRESS: u32 = 0x0600_0000;
    
    const ENGINE_A_BG_VRAM_MASK: u32 = 4 * 128 * 0x400 - 1;

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
        }
    }

    pub fn write_vram_cnt(&mut self, index: usize, value: u8) {
        let bank = Bank::from_index(index);
        let new_cnt = VRAMCNT::new(index, value);

        if self.cnts[index].enabled {
            for addr in self.mapping_ranges[index].clone().step_by(VRAM::MAPPING_LEN) {
                self.mappings.remove(&addr);
            }
            match self.cnts[index].mst {
                0 => {
                    assert!(self.lcdc_enabled[index]);
                    self.lcdc_enabled[index] = false
                },
                1 => for addr in self.mapping_ranges[index].clone().step_by(VRAM::MAPPING_LEN) {
                    self.engine_a_bg[(addr & VRAM::ENGINE_A_BG_VRAM_MASK) as usize / VRAM::MAPPING_LEN] = None;
                },
                2 ..= 5 => todo!(),
                _ => unreachable!(),
            }
        }

        self.cnts[index] = new_cnt;
        if !new_cnt.enabled { return }
        match new_cnt.mst {
            0 => {
                let start_addr = VRAM::LCDC_ADDRESSES[index];
                self.mapping_ranges[index] = start_addr..start_addr + VRAM::BANKS_LEN[index] as u32;
                for addr in self.mapping_ranges[index].clone().step_by(VRAM::MAPPING_LEN) {
                    self.mappings.insert(addr, Mapping::new(bank, start_addr));
                }
                assert!(!self.lcdc_enabled[index]);
                self.lcdc_enabled[index] = true;
            },
            1 => {
                assert!(bank != Bank::H && bank != Bank::I);
                let start_addr = VRAM::ENGINE_A_BG_START_ADDRESS + bank.get_engine_a_offset(new_cnt.offset);
                self.mapping_ranges[index] = start_addr..start_addr + VRAM::BANKS_LEN[index] as u32;
                for (i, addr) in self.mapping_ranges[index].clone().step_by(VRAM::MAPPING_LEN).enumerate() {
                    // TODO: Support overlapping banks
                    assert!(!self.mappings.contains_key(&addr));
                    self.mappings.insert(addr, Mapping::new(bank, start_addr));
                    self.engine_a_bg[(addr & VRAM::ENGINE_A_BG_VRAM_MASK) as usize / VRAM::MAPPING_LEN] = 
                        Some(Mapping::new(bank, (VRAM::MAPPING_LEN * i) as u32));
                }
            },
            2 ..= 5 => todo!(),
            _ => unreachable!(),
        }
    }

    pub fn get_lcdc_bank(&self, bank: u8) -> Option<&Vec<u8>> {
        if self.lcdc_enabled[bank as usize] { Some(&self.banks[bank as usize]) } else { None }
    }

    pub fn _get_engine_a_bg(&self, addr: u32) -> u8 {
        if let Some(mapping) = self.engine_a_bg[addr as usize / VRAM::MAPPING_LEN] {
            self.banks[mapping.bank as usize][(mapping.offset + (addr % VRAM::MAPPING_LEN as u32)) as usize]
        } else { 0 }
    }

    pub fn get_mem(&self, addr: u32) -> Option<(&Vec<u8>, u32)> {
        if let Some(mapping) = self.mappings.get(&(addr & !(VRAM::MAPPING_LEN as u32 - 1))) {
            Some((&self.banks[mapping.bank as usize], addr - mapping.offset))
        } else { None }
    }

    pub fn get_mem_mut(&mut self, addr: u32) -> Option<(&mut Vec<u8>, u32)> {
        if let Some(mapping) = self.mappings.get(&(addr & !(VRAM::MAPPING_LEN as u32 - 1))) {
            Some((&mut self.banks[mapping.bank as usize], addr - mapping.offset))
        } else { None }
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

#[derive(Clone, Copy, Debug)]
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
}
