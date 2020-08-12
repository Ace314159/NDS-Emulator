mod game_db;
mod no_backup;
mod eeprom;
mod flash;

use super::Header;

use no_backup::NoBackup;
use eeprom::{EEPROM, EEPROMSmall, EEPROMNormal};
use flash::Flash;


pub trait Backup {
    fn read(&self) -> u8;
    fn write(&mut self, hold: bool, value: u8);
}

impl dyn Backup {
    pub fn detect_type(header: &Header) -> Box<dyn Backup> {
        let game_code = u32::from_le_bytes(header.game_code);
        if let Some(pos) = Backup::GAME_DB.iter().position(|game_info| game_info.game_code == game_code) {
            let game_info = &Backup::GAME_DB[pos];
            let sram_size = Backup::SRAM_SIZES[game_info.sram_type];
            match game_info.sram_type {
                1 => Box::new(EEPROM::<EEPROMSmall>::new(sram_size)),
                2 ..= 4 => Box::new(EEPROM::<EEPROMNormal>::new(sram_size)),
                5 ..= 8 => Box::new(Flash::new(sram_size)),
                _ => todo!(),
            }
        } else {
            warn!("Game not found in DB!");
            Box::new(NoBackup::new())
        }
    }
}
