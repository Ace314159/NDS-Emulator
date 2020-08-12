mod game_db;
mod no_backup;
mod eeprom;

use super::Header;

use no_backup::NoBackup;


pub trait Backup {
    fn read(&mut self) -> u8;
    fn write(&mut self, value: u8);
}

impl dyn Backup {
    pub fn detect_type(header: &Header) -> Box<dyn Backup> {
        let game_code = u32::from_le_bytes(header.game_code);
        if let Some(_pos) = Backup::GAME_DB.iter().position(|game_info| game_info.game_code == game_code) {
            // TODO
            Box::new(NoBackup::new())
        } else {
            warn!("Game not found in DB!");
            Box::new(NoBackup::new())
        }
    }
}
