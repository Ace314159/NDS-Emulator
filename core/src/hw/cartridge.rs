mod backup;
mod header;
mod key1_encryption;

use std::collections::VecDeque;
use std::convert::TryInto;
use std::fs::File;
use std::ops::Range;

use super::{
    dma,
    interrupt_controller::InterruptRequest,
    scheduler::{Event, Scheduler},
    HW,
};

use header::Header;
use key1_encryption::Key1Encryption;

pub(super) use backup::{Backup, Flash}; // For Firmware

pub struct Cartridge {
    chip_id: u32,
    header: Header,
    rom: Vec<u8>,
    key1_encryption: Key1Encryption,
    // Registers
    pub spicnt: SPICNT,
    romctrl: ROMCTRL,
    command: [u8; 8],
    cur_game_card_word: u32,
    // Data Transfer
    rom_bytes_left: usize,
    game_card_words: VecDeque<u32>,
    // Backup
    backup: Box<dyn Backup>,
}

impl Cartridge {
    const DESTROYED_SECURE_AREA_ID: u32 = 0xE7FFDEFF;
    const DECRYPTED_SECURE_AREA_ID: &'static str = "encryObj";
    const SECURE_AREA_RANGE: Range<usize> = 0x4000..0x8000;
    const SECURE_AREA_SIZE: usize = 0x800;

    pub fn new(rom: Vec<u8>, save_file: File, bios7: &[u8]) -> Self {
        let header = Header::new(&rom);
        let backup = <dyn Backup>::detect_type(&header, save_file);

        Cartridge {
            chip_id: 0x000_00FC2u32, // TODO: Actually Calculate
            header,
            rom,
            key1_encryption: Key1Encryption::new(bios7),
            // Registers
            spicnt: SPICNT::new(),
            romctrl: ROMCTRL::new(),
            command: [0; 8],
            cur_game_card_word: 0,
            // Data Transfer
            rom_bytes_left: 0,
            game_card_words: VecDeque::new(),
            backup,
        }
    }

    pub fn encrypt_secure_area(&mut self) {
        let start = self.header.arm9_rom_offset as usize;
        if !Self::SECURE_AREA_RANGE.contains(&start) {
            return;
        }

        let secure_area_range = || start..start + Self::SECURE_AREA_SIZE;
        let secure_area = &mut self.rom[secure_area_range()];
        let secure_area_32: &[u32] = bytemuck::cast_slice(&secure_area);
        // Check secure area exists
        for i in 0..3 {
            if secure_area_32[i] != Self::DESTROYED_SECURE_AREA_ID {
                return;
            }
        }
        if secure_area[0xC] != 0xFF || secure_area[0xD] != 0xDE {
            return;
        }

        // First 8 bytes over secure area is overwritten by BIOS after decryption, so put correct string
        secure_area[..0x8].copy_from_slice(Self::DECRYPTED_SECURE_AREA_ID.as_bytes());
        let secure_area_32: &mut [u32] =
            bytemuck::cast_slice_mut(&mut self.rom[secure_area_range()]);
        // Level 3 for entire secure area
        self.key1_encryption
            .init_key_code(self.header.game_code, 3, 2);
        for chunk in secure_area_32.chunks_exact_mut(2) {
            self.key1_encryption.encrypt(chunk);
        }
        // Level 2 for first 8 bytes (first 8 bytes encrypted twice)
        self.key1_encryption
            .init_key_code(self.header.game_code, 2, 2);
        self.key1_encryption.encrypt(&mut secure_area_32[..0x8]);
        self.key1_encryption.in_use = false;
    }

    pub fn run_command(&mut self, scheduler: &mut Scheduler, is_arm9: bool) {
        //self.romctrl.key1_gap1_len = 0x10;
        //self.romctrl.key1_gap2_len = 0x10;
        //self.romctrl.key2_encrypt_data = false;
        //self.romctrl.key2_encrypt_cmd = false;
        //self.romctrl.block_busy = false;
        //self.romctrl.data_block_size = 0x4;
        //self.romctrl.resb_release_reset = false;
        assert_eq!(self.rom_bytes_left % 4, 0);
        self.rom_bytes_left = match self.romctrl.data_block_size {
            0 => 0,
            7 => 4,
            _ => {
                assert!(self.romctrl.data_block_size < 7);
                0x100 << self.romctrl.data_block_size
            }
        };
        self.romctrl.block_busy = true;
        self.romctrl.data_word_ready = false;
        self.game_card_words.clear();

        if self.key1_encryption.in_use {
            self.run_encrypted_command();
        } else {
            self.run_unencrypted_command();
        }

        // TODO: Take into account WR bit
        if self.rom_bytes_left == 0 {
            // 8 command bytes transferred
            scheduler.schedule(
                Event::ROMBlockEnded(is_arm9),
                HW::on_rom_block_ended,
                self.transfer_byte_time() * 8,
            );
        } else {
            // 8 command bytes + 4 bytes for word
            scheduler.schedule(
                Event::ROMWordTransfered(is_arm9),
                HW::on_rom_word_transfered,
                self.transfer_byte_time() * (8 + 4),
            );
        }
    }

    fn copy_rom(&mut self, range: Range<usize>) {
        for addr in range.step_by(4) {
            self.game_card_words.push_back(u32::from_le_bytes(
                self.rom[addr..addr + 4].try_into().unwrap(),
            ));
        }
    }

    pub fn run_encrypted_command(&mut self) {
        // Command is in given as big endian, but the decryption works with little endian
        self.command.reverse();
        self.key1_encryption
            .decrypt(bytemuck::cast_slice_mut(&mut self.command));
        self.command.reverse();

        // TODO: Verify command parameters
        match self.command[0] >> 4 {
            0x1 => {
                // 0x910 dummy bytes and 4 bytes of chip id
                // But making them all chip id works anyway
                for _ in 0..self.rom_bytes_left / 4 {
                    self.game_card_words.push_back(self.chip_id);
                }
            }
            0x2 => {
                let addr = (self.command[2] as usize & 0xF0) << (12 - 4)
                    | (self.command[1] as usize) << 8
                    | self.command[0] as usize & 0x0F;
                assert!((0x4000..=0x7000).contains(&addr));
                assert_eq!(addr & 0xFFF, 0);
                assert_eq!(self.rom_bytes_left, 0x1000);
                self.copy_rom(addr..addr + self.rom_bytes_left);
            }
            0x4 => {
                // Endless stream of HIGH-Z bytes?
                for _ in 0..self.rom_bytes_left / 4 {
                    self.game_card_words.push_back(0xFFFF_FFFF);
                }
            }
            0xA => {
                // TODO: Add 3rd mode instead of using same mode for boot and key2 encrypted
                self.key1_encryption.in_use = false;
                // 0x910 dummy bytes followed by KEY2 encrypted 0s
                // Making them all 0s works
                for _ in 0..self.rom_bytes_left / 4 {
                    self.game_card_words.push_back(0);
                }
            }
            _ => {
                warn!(
                    "Unimplemented Encrypted Cartridge Command: {:X?}",
                    self.command
                );
                for _ in 0..self.rom_bytes_left / 4 {
                    self.game_card_words.push_back(0);
                }
            }
        }
    }

    pub fn run_unencrypted_command(&mut self) {
        match self.command[0] {
            0x00 => {
                for byte in self.command[1..].iter() {
                    assert_eq!(*byte, 0)
                }
                assert!(self.rom_bytes_left < 0x1000); // TODO: Support
                self.copy_rom(0..self.rom_bytes_left);
            }
            0x3C => {
                self.key1_encryption
                    .init_key_code(self.header.game_code, 2, 2);
            }
            0xB7 => {
                for byte in self.command[5..].iter() {
                    assert_eq!(*byte, 0)
                }
                let addr = u32::from_be_bytes(self.command[1..=4].try_into().unwrap()) as usize;
                assert!(addr + self.rom_bytes_left < self.rom.len()); // TODO: Handle mirroring later
                let addr = if addr < 0x8000 {
                    0x8000 + (addr & 0x1FFF)
                } else {
                    addr
                };
                let transfer_len = self.rom_bytes_left;
                if addr & 0x1000 != (addr + transfer_len) & 0x1000 {
                    // Crosess 4K boundary
                    let block4k_start = addr & !0xFFF;
                    let block4k_end = block4k_start + 0x1000;
                    let extra_len = transfer_len - (block4k_end - addr);
                    self.copy_rom(addr..block4k_end);
                    self.copy_rom(block4k_start..block4k_start + extra_len);
                } else {
                    self.copy_rom(addr..addr + transfer_len);
                }
            }
            0xB8 => {
                for byte in self.command[1..].iter() {
                    assert_eq!(*byte, 0)
                }
                // Chip ID is repeated
                for _ in 0..self.rom_bytes_left / 4 {
                    self.game_card_words.push_back(self.chip_id);
                }
            }
            0x90 => {
                // Chip ID is repeated
                for _ in 0..self.rom_bytes_left / 4 {
                    self.game_card_words.push_back(self.chip_id);
                }
            }
            0x9F => {
                // Endless stream of HIGH-Z bytes
                for byte in self.command[1..].iter() {
                    assert_eq!(*byte, 0)
                }
                for _ in 0..self.rom_bytes_left / 4 {
                    self.game_card_words.push_back(0xFFFF_FFFF);
                }
            }
            _ => {
                warn!(
                    "Unimplemented Unencrypted Cartridge Command: {:X?}",
                    self.command
                );
                for _ in 0..self.rom_bytes_left / 4 {
                    self.game_card_words.push_back(0);
                }
            }
        };
    }

    pub fn read_gamecard(
        &mut self,
        scheduler: &mut Scheduler,
        is_arm9: bool,
        has_access: bool,
    ) -> u32 {
        if !has_access {
            warn!("No Read Access from Game Card Command");
            return 0;
        }
        if self.romctrl.data_word_ready {
            self.romctrl.data_word_ready = false;
            assert!(self.rom_bytes_left > 0);
            self.rom_bytes_left -= 4;

            if self.rom_bytes_left > 0 {
                // 1 word (4 bytes) transferred
                scheduler.schedule(
                    Event::ROMWordTransfered(is_arm9),
                    HW::on_rom_word_transfered,
                    self.transfer_byte_time() * 4,
                );
            } else {
                scheduler.run_now(Event::ROMBlockEnded(is_arm9), HW::on_rom_block_ended);
            }
        } else {
            warn!("Reading Old Game Card Value");
        }
        self.cur_game_card_word
    }

    pub fn read_spi_data(&self, has_access: bool) -> u8 {
        if !has_access {
            warn!("No Read Access to SPI DATA");
            return 0;
        }
        self.backup.read()
    }

    pub fn read_romctrl(&self, has_access: bool, byte: usize) -> u8 {
        self.romctrl.read(has_access, byte)
    }

    pub fn write_spi_data(&mut self, has_access: bool, value: u8) {
        if !has_access {
            warn!("No Write Access to SPI DATA");
            return;
        }
        self.backup.write(self.spicnt.hold, value);
    }

    pub fn write_command(&mut self, has_access: bool, byte: usize, value: u8) {
        if !has_access {
            warn!("No Write Access to Game Card Command");
            return;
        }
        assert!(byte < 8);
        self.command[byte] = value;
    }

    pub fn write_romctrl(
        &mut self,
        scheduler: &mut Scheduler,
        is_arm9: bool,
        has_access: bool,
        byte: usize,
        value: u8,
    ) {
        if self.romctrl.write(has_access, byte, value) {
            self.run_command(scheduler, is_arm9)
        }
    }

    pub fn chip_id(&self) -> u32 {
        self.chip_id
    }
    pub fn rom(&self) -> &Vec<u8> {
        &self.rom
    }
    pub fn header(&self) -> &Header {
        &self.header
    }

    fn transfer_byte_time(&self) -> usize {
        if self.romctrl.transfer_clk_rate {
            8
        } else {
            5
        }
    }
}

impl HW {
    fn on_rom_word_transfered(&mut self, event: Event) {
        let is_arm9 = match event {
            Event::ROMWordTransfered(is_arm9) => is_arm9,
            _ => unreachable!(),
        };
        self.cartridge.cur_game_card_word = self.cartridge.game_card_words.pop_front().unwrap();
        self.cartridge.romctrl.data_word_ready = true;
        self.run_dmas_single(dma::Occasion::DSCartridge, is_arm9);
    }

    fn on_rom_block_ended(&mut self, event: Event) {
        let is_arm9 = match event {
            Event::ROMBlockEnded(is_arm9) => is_arm9,
            _ => unreachable!(),
        };
        self.cartridge.romctrl.block_busy = false;
        if self.cartridge.spicnt.transfer_ready_irq {
            self.interrupts[(is_arm9) as usize].request |=
                InterruptRequest::GAME_CARD_TRANSFER_COMPLETION;
        }
    }
}

pub struct SPICNT {
    // Registers
    baudrate: u8,
    hold: bool,
    busy: bool,
    slot_mode: bool,
    transfer_ready_irq: bool,
    slot_enable: bool,
}

impl SPICNT {
    pub fn new() -> Self {
        SPICNT {
            baudrate: 0,
            hold: false,
            busy: false,
            slot_mode: false,
            transfer_ready_irq: false,
            slot_enable: false,
        }
    }

    pub fn read(&self, has_access: bool, byte: usize) -> u8 {
        if !has_access {
            warn!("No Read Access to AUX SPI CNT");
            return 0;
        }
        //println!("Reading AUXSPICNT {}", byte);
        match byte {
            0 => (self.busy as u8) << 7 | (self.hold as u8) << 6 | self.baudrate,
            1 => {
                (self.slot_enable as u8) << 7
                    | (self.transfer_ready_irq as u8) << 6
                    | (self.slot_mode as u8) << 5
            }
            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, has_access: bool, byte: usize, value: u8) {
        //println!("Writing AUXSPICNT {}: 0x{:X}", byte, value);
        if !has_access {
            warn!("No Write Access to AUX SPI CNT");
            return;
        }
        match byte {
            0 => {
                self.baudrate = value & 0x3;
                self.hold = value >> 6 & 0x1 != 0;
                self.busy = value >> 7 & 0x1 != 0;
            }
            1 => {
                self.slot_mode = value >> 5 & 0x1 != 0;
                self.transfer_ready_irq = value >> 6 & 0x1 != 0;
                self.slot_enable = value >> 7 & 0x1 != 0;
            }
            _ => unreachable!(),
        }
    }
}

pub struct ROMCTRL {
    key1_gap1_len: u16,
    key2_encrypt_data: bool,
    _key2_apply_seed: bool,
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
            _key2_apply_seed: false,
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
        if !has_access {
            warn!("No Read Access to ROM CTRL");
            return 0;
        }
        //println!("Reading AUXROMCTRL {}", byte);
        // TODO: Are bits 13 and 14 the same
        match byte {
            0 => self.key1_gap1_len as u8,
            1 => {
                (self.key2_encrypt_data as u8) << 6
                    | (self.key2_encrypt_data as u8) << 5
                    | (self.key1_gap1_len >> 8) as u8
            }
            2 => {
                (self.data_word_ready as u8) << 7
                    | (self.key2_encrypt_cmd as u8) << 6
                    | (self.key1_gap2_len) as u8
            }
            3 => {
                (self.block_busy as u8) << 7
                    | (self.wr as u8) << 6
                    | (self.resb_release_reset as u8) << 5
                    | (self.key1_gap_clks as u8) << 4
                    | (self.transfer_clk_rate as u8) << 3
                    | (self.data_block_size as u8)
            }
            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, has_access: bool, byte: usize, value: u8) -> bool {
        if !has_access {
            warn!("No Write Access to ROM CTRL");
            return false;
        }
        //println!("Writing AUXROMCTRL {}: 0x{:X}", byte, value);
        match byte {
            0 => self.key1_gap1_len = self.key1_gap1_len & !0xFF | value as u16,
            1 => {
                self.key1_gap1_len = self.key1_gap1_len & !0x1F00 | (value as u16 & 0x1F) << 4;
                self.key2_encrypt_data = value >> 5 & 0x1 != 0;
                self._key2_apply_seed = value >> 7 & 0x1 != 0;
            }
            2 => {
                self.key1_gap2_len = value & 0x3F;
                self.key2_encrypt_cmd = value >> 6 & 0x1 != 0;
                // Data-word Status (data_word_ready) is read-only
            }
            3 => {
                self.data_block_size = value & 0x7;
                self.transfer_clk_rate = value >> 3 & 0x1 != 0;
                self.key1_gap_clks = value >> 4 & 0x1 != 0;
                self.resb_release_reset = self.resb_release_reset || value >> 5 & 0x1 != 0; // Cannot be cleared once set
                self.wr = value >> 6 & 0x1 != 0;
                return value & 0x80 != 0; // Block Start
            }
            _ => unreachable!(),
        }
        false
    }
}
