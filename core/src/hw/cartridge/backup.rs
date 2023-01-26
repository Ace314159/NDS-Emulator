mod eeprom;
mod flash;
mod game_db;
mod no_backup;

use memmap::{MmapMut, MmapOptions};
use std::{fs::File, io::Write};

use super::Header;

use eeprom::{EEPROMNormal, EEPROMSmall, EEPROM};
pub use flash::Flash;
use no_backup::NoBackup;

pub trait Backup {
    fn read(&self) -> u8;
    fn write(&mut self, hold: bool, value: u8);
}

impl dyn Backup {
    pub fn detect_type(header: &Header, save_file: File) -> Box<dyn Backup> {
        if let Some(pos) = <dyn Backup>::GAME_DB
            .iter()
            .position(|game_info| game_info.game_code == header.game_code)
        {
            let game_info = &<dyn Backup>::GAME_DB[pos];
            let sram_size = <dyn Backup>::SRAM_SIZES[game_info.sram_type];
            match game_info.sram_type {
                1 => Box::new(EEPROM::<EEPROMSmall>::new(save_file, sram_size)),
                2..=4 => Box::new(EEPROM::<EEPROMNormal>::new(save_file, sram_size)),
                5..=8 => Box::new(Flash::new_backup(save_file, sram_size)),
                _ => todo!(),
            }
        } else {
            warn!("Game not found in DB!");
            Box::new(NoBackup::new())
        }
    }

    fn mmap(save_file: File, default_val: u8, size: usize) -> MmapMut {
        let mut save_file = save_file;
        if save_file.metadata().unwrap().len() as usize != size {
            save_file.write_all(&vec![default_val; size]).unwrap();
        }

        unsafe { MmapOptions::new().map_mut(&save_file).unwrap() }
    }
}
