pub struct Firmware {
    mem: Vec<u8>,
    state: State,
    read_value: u8,
}

impl Firmware {
    pub fn new(mem: Vec<u8>) -> Self {
        Firmware {
            mem,
            state: State::ReadInstr,
            read_value: 0,
        }
    }

    pub fn read(&self) -> u8 {
        self.read_value
    }

    pub fn write(&mut self, value: u8) {
        match self.state {
            State::ReadInstr => {
                self.state = State::HandleInstr(Instr::from_byte(value))
            },

            State::HandleInstr(Instr::Read(3, addr)) => {
                assert_eq!(value, 0);
                self.read_value = self.mem[addr as usize];
                self.state = State::HandleInstr(Instr::ContinuousRead(addr + 1))
            },
            State::HandleInstr(Instr::Read(i, addr)) => {
                self.state = State::HandleInstr(Instr::Read(i + 1, addr << 8 | value as u32))
            },
            State::HandleInstr(Instr::ContinuousRead(addr)) => {
                if value == 0 {
                    self.read_value = self.mem[addr as usize];
                    self.state = State::HandleInstr(Instr::ContinuousRead(addr + 1))
                } else {
                    self.state = State::ReadInstr;
                    self.write(value);
                }
            },
        };
    }
}

#[derive(Clone, Copy, Debug)]
enum State {
    ReadInstr,
    HandleInstr(Instr),
}

#[derive(Clone, Copy, Debug)]
enum Instr {
    Read(usize, u32),
    ContinuousRead(u32),
}

impl Instr {
    pub fn from_byte(value: u8) -> Self {
        match value {
            0x03 => Self::Read(0, 0),
            _ => todo!(),
        }
    }
}
