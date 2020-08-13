mod game_db;
mod no_backup;
mod eeprom;
mod flash;

use std::fs;
use std::path::PathBuf;

use super::Header;

use no_backup::NoBackup;
use eeprom::{EEPROM, EEPROMSmall, EEPROMNormal};
use flash::Flash;


pub trait Backup {
    fn read(&self) -> u8;
    fn write(&mut self, hold: bool, value: u8);
    
    fn mem(&self) -> &Vec<u8>;
    fn save_file(&self) -> &PathBuf;
    fn dirty(&mut self) -> bool;
}

impl dyn Backup {
    pub fn detect_type(header: &Header, save_file: PathBuf) -> Box<dyn Backup> {
        let game_code = u32::from_le_bytes(header.game_code);
        if let Some(pos) = Backup::GAME_DB.iter().position(|game_info| game_info.game_code == game_code) {
            let game_info = &Backup::GAME_DB[pos];
            let sram_size = Backup::SRAM_SIZES[game_info.sram_type];
            match game_info.sram_type {
                1 => Box::new(EEPROM::<EEPROMSmall>::new(save_file, sram_size)),
                2 ..= 4 => Box::new(EEPROM::<EEPROMNormal>::new(save_file, sram_size)),
                5 ..= 8 => Box::new(Flash::new(save_file, sram_size)),
                _ => todo!(),
            }
        } else {
            warn!("Game not found in DB!");
            Box::new(NoBackup::new())
        }
    }

    fn get_initial_mem(save_file: &PathBuf, default_val: u8, size: usize) -> Vec<u8> {
        if let Ok(mem) = fs::read(save_file) {
            if mem.len() == size { mem } else { vec![default_val; size] }
        } else { vec![default_val; size] }
    }

    pub fn save(&mut self) {
        if self.dirty() {
            fs::write(self.save_file(), self.mem())
            .unwrap_or_else(|err| warn!("Unable to Save to File: {}!", err))
        }
    }
}
