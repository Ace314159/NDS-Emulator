use std::collections::HashMap;

pub struct VRAM {
    mem: [Vec<u8>; 9],
    mappings: HashMap<u32, Mapping>, // TODO: Switch to using array
    mapping_ranges: [(u32, u32); 9],
}

impl VRAM {
    const MST_MASKS: [u8; 9] = [0x3, 0x3, 0x7, 0x7, 0x7, 0x7, 0x7, 0x3, 0x3];
    const OFS_MASKS: [u8; 9] = [0x3, 0x3, 0x3, 0x3, 0x0, 0x3, 0x3, 0x0, 0x0];

    const BANKS_LEN: [usize; 9] = [128 * 0x400, 128 * 0x400, 128 * 0x400, 128 * 0x400,
        64 * 0x400, 16 * 0x400, 16 * 0x400, 32 * 0x400, 16 * 0x400];
    const MAPPING_LEN: usize = 16 * 0x400;

    const LCD_ADDRESSES: [u32; 9] = [0x0680_0000, 0x0682_0000, 0x0684_0000, 0x0686_0000,
        0x0688_0000, 0x0689_0000, 0x0689_4000, 0x0689_8000, 0x068A_0000];

    pub fn new() -> Self {
        VRAM {
            mem: [
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
            mapping_ranges: [(0, 0); 9],
        }
    }

    pub fn write_vram_cnt(&mut self, index: usize, value: u8) {
        let mst = value & VRAM::MST_MASKS[index];
        let _offset = value >> 3 & VRAM::OFS_MASKS[index];
        let enable = value >> 7 & 0x1 != 0;

        if self.mapping_ranges[index].0 != 0 {
            let range = self.mapping_ranges[index].0..self.mapping_ranges[1].1;
            for addr in range.step_by(VRAM::MAPPING_LEN) {
                self.mappings.remove(&addr);
            }
        }

        if !enable { return }
        match mst {
            0 => {
                let start_addr = VRAM::LCD_ADDRESSES[index];
                self.mapping_ranges[index] = (start_addr, start_addr + VRAM::BANKS_LEN[index] as u32);
                let range = self.mapping_ranges[index].0..self.mapping_ranges[index].1;
                for addr in range.step_by(VRAM::MAPPING_LEN) {
                    self.mappings.insert(addr, Mapping::new(index, start_addr));
                }
            },
            1 ..= 5 => todo!(),
            _ => unreachable!(),
        }
    }

    pub fn get_mem(&self, addr: u32) -> Option<(&Vec<u8>, u32)> {
        if let Some(mapping) = self.mappings.get(&(addr & !(VRAM::MAPPING_LEN as u32 - 1))) {
            Some((&self.mem[mapping.index], addr - mapping.offset))
        } else { None }
    }

    pub fn get_mem_mut(&mut self, addr: u32) -> Option<(&mut Vec<u8>, u32)> {
        if let Some(mapping) = self.mappings.get(&(addr & !(VRAM::MAPPING_LEN as u32 - 1))) {
            Some((&mut self.mem[mapping.index], addr - mapping.offset))
        } else { None }
    }
}

#[derive(Clone, Copy)]
struct Mapping {
    index: usize,
    offset: u32,
}

impl Mapping {
    pub fn new(index: usize, offset: u32) -> Mapping {
        Mapping {
            index,
            offset,
        }
    }
}
