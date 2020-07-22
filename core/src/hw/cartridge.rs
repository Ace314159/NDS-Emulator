use std::convert::TryInto;

use super::scheduler::{EventType, Scheduler};

pub struct Cartridge {
    rom: Vec<u8>,
    pub spicnt: SPICNT,
    romctrl: ROMCTRL,
    command: [u8; 8],
    gamecard_bytes: [u8; 4],
}

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Self {
        Cartridge {
            rom,
            spicnt: SPICNT::new(),
            romctrl: ROMCTRL::new(),
            command: [0; 8],
            gamecard_bytes: [0; 4],
        }
    }

    pub fn run_command(&mut self, chip_id: u32) -> bool {
        self.romctrl.block_busy = false;
        self.romctrl.data_word_ready = true; // TODO: Take some time to set this
        match self.command[0] {
            0xB7 => {
                for byte in self.command[5..8].iter() { assert_eq!(*byte, 0) }
                let addr = u32::from_be_bytes(self.command[1..=4].try_into().unwrap()) as usize;
                self.gamecard_bytes.copy_from_slice(&self.rom[addr..addr + 4]);
            },
            0xB8 => {
                for byte in self.command[1..8].iter() { assert_eq!(*byte, 0) }
                for i in 0..4 { self.gamecard_bytes[i] = (chip_id >> i * 8) as u8 };
            },
            _ => todo!(),
        };
        self.spicnt.transfer_ready_irq
    }

    pub fn read_gamecard(&self, byte: usize) -> u8 {
        println!("Reading from game card");
        assert!(byte < 4);
        self.gamecard_bytes[byte]
    }

    pub fn read_spi_data(&self, has_access: bool) -> u8 {
        if !has_access { warn!("No Read Access to SPI DATA"); return 0 }
        println!("Reading from AUX SPI DATA");
        0
    }

    pub fn read_romctrl(&self, has_access: bool, byte: usize) -> u8 { self.romctrl.read(has_access, byte) }

    pub fn write_spi_data(&mut self, has_access: bool, value: u8) {
        if !has_access { warn!("No Write Access to SPI DATA"); return }
        println!("Writing to AUX SPI DATA: 0x{:X}", value);
    }

    pub fn write_romctrl(&mut self, scheduler: &mut Scheduler, is_arm7: bool, has_access: bool, byte: usize, value: u8) {
        self.romctrl.write(scheduler, is_arm7, has_access, byte, value);
    }

    pub fn write_command(&mut self, has_access: bool, byte: usize, value: u8) {
        if !has_access { warn!("No Write Access to Gamecard Command"); return }
        assert!(byte < 8);
        println!("Writing Command Byte {}: 0x{:X}", byte, value);
        self.command[byte] = value;
    }

    pub fn rom(&self) -> &Vec<u8> { &self.rom }
}

pub struct SPICNT {
    baudrate: u8,
    hold_chipselect: bool,
    busy: bool,
    slot_mode: bool,
    transfer_ready_irq: bool,
    slot_enable: bool,
}

impl SPICNT {
    pub fn new() -> Self {
        SPICNT {
            baudrate: 0,
            hold_chipselect: false,
            busy: false,
            slot_mode: false,
            transfer_ready_irq: false,
            slot_enable: false,
        }
    }

    pub fn read(&self, has_access: bool, byte: usize) -> u8 {
        if !has_access { warn!("No Read Access to AUX SPI CNT"); return 0 }
        println!("Reading AUXSPICNT {}", byte);
        match byte {
            0 => (self.busy as u8) << 7 | (self.hold_chipselect as u8) << 6 | self.baudrate,
            1 => (self.slot_enable as u8) << 7 | (self.transfer_ready_irq as u8) << 6 | (self.slot_mode as u8) << 5,
            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, has_access: bool, byte: usize, value: u8) {
        println!("Writing AUXSPICNT {}: 0x{:X}", byte, value);
        if !has_access { warn!("No Write Access to AUX SPI CNT"); return }
        match byte {
            0 => {
                self.baudrate = value & 0x3;
                self.hold_chipselect = value >> 6 & 0x1 != 0;
                self.busy = value >> 7 & 0x1 != 0;
            },
            1 => {
                self.slot_mode = value >> 5 & 0x1 != 0;
                self.transfer_ready_irq = value >> 6 & 0x1 != 0;
                self.slot_enable = value >> 7 & 0x1 != 0;
            },
            _ => unreachable!(),
        }
    }
}

pub struct ROMCTRL {
    key1_gap1_len: u16,
    key2_encrypt_data: bool,
    key2_apply_seed: bool,
    key1_gap2_len: u8,
    key2_encrypt_cmd: bool,
    data_word_ready: bool,
    data_block_size: u8,
    transfer_clk_rate: bool,
    key1_gap_clks: bool,
    resb_release_reset: bool,
    wr: bool,
    block_busy: bool,
}

impl ROMCTRL {
    pub fn new() -> Self {
        ROMCTRL {
            key1_gap1_len: 0,
            key2_encrypt_data: false,
            key2_apply_seed: false,
            key1_gap2_len: 0,
            key2_encrypt_cmd: false,
            data_word_ready: false,
            data_block_size: 0,
            transfer_clk_rate: false,
            key1_gap_clks: false,
            resb_release_reset: false,
            wr: false,
            block_busy: false,
        }
    }

    pub fn read(&self, has_access: bool, byte: usize) -> u8 {
        if !has_access { warn!("No Read Access to ROM CTRL"); return 0 }
        println!("Reading AUXROMCTRL {}", byte);
        // TODO: Are bits 13 and 14 the same
        match byte {
            0 => self.key1_gap1_len as u8,
            1 => (self.key2_encrypt_data as u8) << 6 | (self.key2_encrypt_data as u8) << 5 | (self.key1_gap1_len >> 8) as u8,
            2 => (self.data_word_ready as u8) << 7 | (self.key2_encrypt_cmd as u8) << 6 | (self.key1_gap2_len) as u8,
            3 => (self.block_busy as u8) << 7 | (self.wr as u8) << 6 | (self.resb_release_reset as u8) << 5 |
                (self.key1_gap_clks as u8) << 4 | (self.transfer_clk_rate as u8) << 3 | (self.data_block_size as u8),
            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, scheduler: &mut Scheduler, is_arm7: bool, has_access: bool, byte: usize, value: u8) {
        if !has_access { warn!("No Write Access to ROM CTRL"); return }
        println!("Writing AUXROMCTRL {}: 0x{:X}", byte, value);
        match byte {
            0 => self.key1_gap1_len = self.key1_gap1_len & !0xFF | value as u16,
            1 => {
                self.key1_gap1_len = self.key1_gap1_len & !0x1F00 | (value as u16 & 0x1F) << 4;
                self.key2_encrypt_data = value >> 5 & 0x1 != 0;
                self.key2_apply_seed = value >> 7 & 0x1 != 0;
            },
            2 => {
                self.key1_gap2_len = value & 0x3F;
                self.key2_encrypt_cmd = value >> 5 & 0x1 != 0;
                // Data-word Status (data_word_ready) is read-only
            },
            3 => {
                self.data_block_size = value & 0x7;
                self.transfer_clk_rate = value >> 3 & 0x1 != 0;
                self.key1_gap_clks = value >> 4 & 0x1 != 0;
                self.resb_release_reset = self.resb_release_reset || value >> 5 & 0x1 != 0; // Cannot be cleared once set
                self.wr = value >> 6 & 0x1 != 0;
                if value & 0x80 != 0 { // Block Start
                    // TODO: Add Delay
                    self.block_busy = true;
                    scheduler.run_now(EventType::RunGameCardCommand(is_arm7));
                }
            },
            _ => unreachable!(),
        }
    }
}
