use super::Backup;

pub struct NoBackup {}

impl Backup for NoBackup {
    fn read(&self) -> u8 {
        0
    }
    fn write(&mut self, _hold: bool, _value: u8) {}
}

impl NoBackup {
    pub fn new() -> Self {
        NoBackup {}
    }
}
