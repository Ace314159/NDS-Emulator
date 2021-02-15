use std::path::PathBuf;

use super::Backup;

pub struct NoBackup {}

impl Backup for NoBackup {
    fn read(&self) -> u8 {
        0
    }
    fn write(&mut self, _hold: bool, _value: u8) {}

    fn mem(&self) -> &Vec<u8> {
        unreachable!()
    }
    fn save_file(&self) -> &PathBuf {
        unreachable!()
    }
    fn dirty(&mut self) -> bool {
        false
    }
}

impl NoBackup {
    pub fn new() -> Self {
        NoBackup {}
    }
}
