use std::path::PathBuf;

use super::Backup;

pub struct Flash {
    save_file: PathBuf,
    mem: Vec<u8>,
    dirty: bool,

    mode: Mode,
    value: u8,
    // Status Reg
    write_enable: bool,
    
}

impl Flash {
    pub fn new_backup(save_file: PathBuf, size: usize) -> Self {
        Flash {
            mem: Backup::get_initial_mem(&save_file, 0xFF, size),
            save_file,
            dirty: false,

            mode: Mode::ReadInstr,
            value: 0,
            // Status Reg
            write_enable: false,
        }
    }

    pub fn new_firmware(firmware: Vec<u8>) -> Self {
        Flash {
            mem: firmware,
            save_file: PathBuf::new(),
            dirty: false,

            mode: Mode::ReadInstr,
            value: 0,
            // Status Reg
            write_enable: false,
        }
    }

    fn set_instr(&mut self, instr: Instr) -> Mode {
        match instr {
            Instr::IR => Mode::ReadInstr, // TODO: Actually implement IR
            Instr::WREN => {
                self.write_enable = true;
                Mode::ReadInstr
            },
            _ => Mode::HandleInstr(instr),
        }
    }

    fn handle_instr(&mut self, instr: Instr, value: u8) -> Mode {
        match instr {
            Instr::IR => unreachable!(),

            Instr::READ(0, addr) => {
                assert_eq!(value, 0);
                self.value = self.mem[addr];
                Mode::HandleInstr(Instr::READ(0, addr + 1))
            },
            Instr::READ(addr_bytes_left, addr) => {
                Mode::HandleInstr(Instr::READ(addr_bytes_left - 1, addr << 8 | value as usize))
            },

            Instr::RDSR => {
                assert_eq!(value, 0);
                // TODO: Figure out if in Progress needs to be emulated
                self.value = (self.write_enable as u8) << 1;
                Mode::ReadInstr
            },

            Instr::WREN => unreachable!(),

            Instr::PW(0, addr) => {
                self.dirty = true;
                self.value = self.mem[addr];
                self.mem[addr] = value;
                Mode::HandleInstr(Instr::PW(0, addr + 1))
            },
            Instr::PW(addr_bytes_left, addr) => {
                Mode::HandleInstr(Instr::PW(addr_bytes_left - 1, addr << 8 | value as usize))
            },
        }
    }

    pub fn deselect(&mut self) {
        self.mode = Mode::ReadInstr;
    }
}

impl Backup for Flash {
    fn read(&self) -> u8 {
        self.value
    }

    fn write(&mut self, hold: bool, value: u8) {
        self.mode = match self.mode {
            Mode::ReadInstr => self.set_instr(Instr::get(value)),
            Mode::HandleInstr(instr) => self.handle_instr(instr, value),
        };
        if !hold { self.mode = Mode::ReadInstr }
    }

    fn mem(&self) -> &Vec<u8> { &self.mem }
    fn save_file(&self) -> &PathBuf { &self.save_file }
    fn dirty(&mut self) -> bool { let old = self.dirty; self.dirty = false; old }
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    ReadInstr,
    HandleInstr(Instr),
}

#[derive(Clone, Copy, Debug)]
enum Instr {
    IR,
    READ(usize, usize),
    RDSR, // Read Status Register
    WREN, // Write Enable
    PW(usize, usize), // Page Write
}

impl Instr {
    fn get(value: u8) -> Self {
        match value {
            0x00 => Instr::IR,
            0x08 => Instr::IR,
            0x03 => Instr::READ(3, 0),
            0x05 => Instr::RDSR,
            0x06 => Instr::WREN,
            0x0A => Instr::PW(3, 0),
            _ => unimplemented!("Flash Instr: 0x{:X}", value),
        }
    }
}
