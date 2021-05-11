use super::{mem::IORegister, scheduler::Scheduler};

pub struct RTC {
    // Register
    data: bool,
    sck: bool,
    cs: bool,
    sck_write: bool,
    data_write: bool,
    cs_write: bool,

    mode: Mode,
    last_byte: bool,
    date_time: DateTime,
}

impl RTC {
    const COMMAND_CODE: u8 = 0b0110;

    pub fn new() -> Self {
        RTC {
            data: false,
            sck: false,
            cs: false,
            sck_write: false,
            data_write: false,
            cs_write: false,

            mode: Mode::StartCmd(false),
            last_byte: false,
            date_time: DateTime::new(),
        }
    }

    fn read_parameter(&mut self, parameter: Parameter) -> (u8, Parameter) {
        let value = match parameter {
            Parameter::StatusReg1 => {
                self.last_byte = true;
                (self.date_time.read_status_reg1(), Parameter::StatusReg1)
            },
            Parameter::StatusReg2 => {
                self.last_byte = true;
                (self.date_time.read_status_reg2(), Parameter::StatusReg2)
            },
            Parameter::DateTime(byte) => {
                self.last_byte = byte == 7 - 1;
                (self.date_time.read(byte), Parameter::DateTime(byte + 1))
            },
            Parameter::Time(byte) => {
                self.last_byte = byte == 3 - 1;
                (self.date_time.read(byte + 4), Parameter::Time(byte + 1))
            },
            Parameter::Alarm1FreqDuty(0) if self.date_time.int_mode & 0x1 == 0 => { // TODO: Figure out
                self.last_byte = true;
                (self.date_time.steady_int, Parameter::Alarm1FreqDuty(0))
            },
            Parameter::Alarm1FreqDuty(byte) => {
                self.last_byte = byte == 3 - 1;
                (self.date_time.alarm1.read(byte), Parameter::Alarm1FreqDuty(byte + 1))
            },
            Parameter::Alarm2(byte) => {
                self.last_byte = byte == 3 - 1;
                (self.date_time.alarm2.read(byte), Parameter::Alarm2(byte + 1))
            },
            Parameter::ClockAdjust => {
                self.last_byte = true;
                (self.date_time.clock_adjust, Parameter::ClockAdjust)
            }
        };
        value
    }

    fn write_parameter(&mut self, parameter: Parameter, value: u8) -> Parameter {
        match parameter {
            Parameter::StatusReg1 => {
                self.date_time.write_status_reg1(value);
                self.last_byte = true;
                Parameter::StatusReg1
            },
            Parameter::StatusReg2 => {
                self.date_time.write_status_reg2(value);
                self.last_byte = true;
                Parameter::StatusReg2
            },
            Parameter::DateTime(byte) => {
                self.date_time.write(byte, value);
                self.last_byte = byte == 7 - 1;
                Parameter::DateTime(byte + 1)
            },
            Parameter::Time(byte) => {
                self.date_time.write(byte + 4, value);
                self.last_byte = byte == 3 - 1;
                Parameter::Time(byte + 1)
            },
            Parameter::Alarm1FreqDuty(0) if self.date_time.int_mode & 0x1 == 0 => { // TODO: Figure out
                self.date_time.steady_int = value;
                self.last_byte = true;
                Parameter::Alarm1FreqDuty(0)
            },
            Parameter::Alarm1FreqDuty(byte) => {
                self.date_time.alarm1.write(byte, value);
                self.last_byte = byte == 3 - 1;
                Parameter::Alarm1FreqDuty(byte + 1)
            },
            Parameter::Alarm2(byte) => {
                self.date_time.alarm2.write(byte, value);
                self.last_byte = byte == 3 - 1;
                Parameter::Alarm2(byte + 1)
            },
            Parameter::ClockAdjust => {
                self.date_time.clock_adjust = value;
                self.last_byte = true;
                Parameter::ClockAdjust
            },
        }
    }
}

impl IORegister for RTC {
    fn read(&self, byte: usize) -> u8 {
        if byte == 1 { return 0 }

        let cs = if !self.cs_write { self.cs as u8 } else { 0 };
        let sck = if !self.sck_write { self.sck_write as u8 } else { 0 };
        let data = if !self.data_write { self.data as u8 } else { 0 };
        (self.cs_write as u8) << 6 | (self.sck_write as u8) << 5 | (self.data_write as u8) << 4 |
            cs << 2 | sck << 1 | data << 0
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        if byte == 1 { return }

        let prev_sck = self.sck;
        self.cs_write = value >> 6 & 0x1 != 0;
        self.sck_write = value >> 5 & 0x1 != 0;
        assert!(self.cs_write && self.sck_write);
        self.data_write = value >> 4 & 0x1 != 0;
        self.cs = value >> 2 & 0x1 != 0;
        self.sck = value >> 1 & 0x1 != 0;
        self.data = value >> 0 & 0x1 != 0;

        self.mode = match self.mode {
            Mode::StartCmd(false) => {
                assert!(!self.cs);
                Mode::StartCmd(true)
            },
            Mode::StartCmd(true) if self.cs && self.sck => Mode::SetCmd(0, 0),
            Mode::StartCmd(true) => self.mode,

            Mode::SetCmd(command, 7) if prev_sck && !self.sck => {
                assert!(self.data_write);
                let command = command << 1 | self.data as u8;
                assert_eq!(command >> 4 & 0xF, RTC::COMMAND_CODE);
                
                let parameter = Parameter::from(command >> 1 & 0x7);
                let (parameter, access_type) = if command & 0x1 != 0 {
                    let (parameter_byte, next_parameter) = self.read_parameter(parameter);
                    (next_parameter, AccessType::Read(parameter_byte, 0))
                } else { (parameter, AccessType::Write(0, 0)) };
                Mode::ExecCmd(parameter, access_type)
            },
            Mode::SetCmd(command, bit) if prev_sck && !self.sck => {
                assert!(self.cs && self.data_write);
                Mode::SetCmd(command << 1 | self.data as u8, bit + 1)
            },
            Mode::SetCmd(_, _) => self.mode,

            Mode::ExecCmd(parameter, AccessType::Read(byte, 7)) if prev_sck && !self.sck => {
                let done = self.last_byte;
                self.data = byte & 0x1 != 0;
                if done { Mode::EndCmd } else {
                    let (parameter_byte, next_parameter) = self.read_parameter(parameter);
                    Mode::ExecCmd(next_parameter, AccessType::Read(parameter_byte, 0))
                }
            },
            Mode::ExecCmd(parameter, AccessType::Read(byte, bit)) if prev_sck && !self.sck => {
                self.data = byte & 0x1 != 0;
                Mode::ExecCmd(parameter, AccessType::Read(byte >> 1, bit + 1))
            },
            Mode::ExecCmd(_, AccessType::Read(_, _)) => self.mode,

            Mode::ExecCmd(parameter, AccessType::Write(byte, 7)) if prev_sck && !self.sck => {
                let done = self.last_byte;
                self.write_parameter(parameter, byte | (self.data as u8) << 7);
                if done { Mode::EndCmd } else {
                    Mode::ExecCmd(parameter, AccessType::Write(byte + 1, 0))
                }
            },
            Mode::ExecCmd(parameter, AccessType::Write(byte, bit)) if prev_sck && !self.sck =>
                Mode::ExecCmd(parameter, AccessType::Write(byte | (self.data as u8) << bit, bit + 1)),
            Mode::ExecCmd(_, AccessType::Write(_, _)) => self.mode,

            Mode::EndCmd if !self.cs => Mode::StartCmd(false),
            Mode::EndCmd => self.mode,
        };
    }
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    StartCmd(bool),
    SetCmd(u8, usize),
    ExecCmd(Parameter, AccessType),
    EndCmd,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Parameter {
    StatusReg1,
    StatusReg2,
    DateTime(u8),
    Time(u8),
    Alarm1FreqDuty(u8),
    Alarm2(u8),
    ClockAdjust,
}

impl Parameter {
    pub fn from(value: u8) -> Self {
        match value {
            0 => Parameter::StatusReg1,
            1 => Parameter::StatusReg2,
            2 => Parameter::DateTime(0),
            3 => Parameter::Time(0),
            4 => Parameter::Alarm1FreqDuty(0),
            5 => Parameter::Alarm2(0),
            6 => Parameter::ClockAdjust,
            _ => panic!("Invalid RTC Command Parameter {}", value),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum AccessType {
    Read(u8, usize),
    Write(u8, usize),
}

struct DateTime {
    // Status Reg 1
    is_24h: bool,
    gp_bits1: u8,
    // Status Reg 2
    int_mode: u8,
    gp_bits2: u8,
    int2_enable: bool,
    test_mode: bool,
    // Date
    year: BCD,
    month: BCD,
    day: BCD,
    day_of_week: BCD,
    // Time
    is_pm: bool,
    hour: BCD,
    minute: BCD,
    second: BCD,
    // Alarms
    alarm1: AlarmReg,
    alarm2: AlarmReg,
    steady_int: u8,
    // Misc
    clock_adjust: u8,
}

impl DateTime {
    pub fn new() -> DateTime {
        DateTime {
            // Status Reg 1
            is_24h: false,
            gp_bits1: 0,
            // Status Reg 2
            int_mode: 0,
            gp_bits2: 0,
            int2_enable: false,
            test_mode: false,
            // Date
            year: BCD::new(0x0, 0x99),
            month: BCD::new(0x1, 0x12),
            day: BCD::new(0x1, 0x30),
            day_of_week: BCD::new(0x1, 0x07),
            // Time
            is_pm: false,
            hour: BCD::new(0x0, 0x23),
            minute: BCD::new(0x0, 0x59),
            second: BCD::new(0x0, 0x59),
            // Alarms
            alarm1: AlarmReg::new(),
            alarm2: AlarmReg::new(),
            steady_int: 0,
            // Misc
            clock_adjust: 0,
        }
    }

    fn read(&self, byte: u8) -> u8 {
        match byte {
            0 => self.year.value(),
            1 => self.month.value(),
            2 => self.day.value(),
            3 => self.day_of_week.value(),
            4 => {
                let hour = self.hour.value();
                let bit_6 = if self.is_24h { hour >= 0x12 } else { self.is_pm };
                (bit_6 as u8) << 6 | hour
            },
            5 => self.minute.value(),
            6 => self.second.value(),
            _ => unreachable!(),
        }
    }

    fn read_status_reg1(&self) -> u8 {
        self.gp_bits1 << 2 | (self.is_24h as u8) << 1
    }

    fn read_status_reg2(&self) -> u8 {
        (self.test_mode as u8) << 7 | (self.int2_enable as u8) << 6 | (self.gp_bits2 << 4) | (self.int_mode)
    }

    fn write(&mut self, byte: u8, value: u8) {
        match byte {
            0 => self.year.set_value(value),
            1 => self.month.set_value(value),
            2 => self.day.set_value(value),
            3 => self.day_of_week.set_value(value),
            4 => {
                self.hour.set_value(value);
                if !self.is_24h { self.is_pm = value >> 6 & 0x1 != 0 };
            },
            5 => self.minute.set_value(value),
            6 => self.second.set_value(value),
            _ => unreachable!(),
        }
    }

    fn write_status_reg1(&mut self, value: u8) {
        if value & 0x1 != 0 {
            todo!("RTC Reset");
        }
        self.is_24h = value >> 1 & 0x1 != 0;
        self.gp_bits1 = value >> 2 & 0x3;
    }

    fn write_status_reg2(&mut self, value: u8) {
        self.int_mode = value & 0x3;
        self.gp_bits2 = value >> 3 & 0x3;
        self.int2_enable = value >> 6 & 0x1 != 0;
        self.test_mode = value >> 7 & 0x1 != 0;
    }
}

struct AlarmReg {
    // Day
    day: u8,
    cmp_spec_day: bool,
    // Hour
    hour: u8,
    is_pm: bool,
    cmp_spec_hour: bool,
    // Min
    min: u8,
    cmp_spec_min: bool,
}

impl AlarmReg {
    pub fn new() -> Self {
        AlarmReg {
            // Day
            day: 0,
            cmp_spec_day: false,
            // Hour
            hour: 0,
            is_pm: false,
            cmp_spec_hour: false,
            // Min
            min: 0,
            cmp_spec_min: false,
        }
    }

    pub fn read(&self, byte: u8) -> u8 {
        match byte {
            0 => (self.cmp_spec_day as u8) << 7 | self.day,
            1 => (self.cmp_spec_hour as u8) << 7 | (self.is_pm as u8) << 6 | self.hour,
            2 => (self.cmp_spec_min as u8) << 7 | self.min,
            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, byte: u8, value: u8) {
        match byte {
            0 => {
                self.cmp_spec_day = value >> 7 & 0x1 != 0;
                self.day = value & 0x7;
            },
            1 => {
                self.cmp_spec_hour = value >> 7 & 0x1 != 0;
                self.is_pm = value >> 6 & 0x1 != 0;
                self.hour = value & 0x3F;
            },
            2 => {
                self.cmp_spec_min = value >> 7 & 0x1 != 0;
                self.min = value & 0x7F;
            },
            _ => unreachable!(),
        }
    }
}

struct BCD {
    initial: u8,
    value: u8,
    max: u8,
}

impl BCD {
    pub fn new(initial: u8, max: u8) -> BCD {
        BCD {
            initial,
            value: initial,
            max,
        }
    }

    pub fn inc(&mut self) -> bool {
        self.inc_with_max(self.max)
    }

    pub fn inc_with_max(&mut self, max: u8) -> bool {
        self.value += 1;
        if self.value > max {
            self.value = self.initial;
            assert!(self.value & 0xF < 0xA && self.value >> 4 < 0xA);
            true
        } else {
            if self.value & 0xF > 0x9 {
                // Shouldn't need to check overflow on upper nibble
                self.value = (self.value & 0xF0) + 0x10;
            }
            assert!(self.value & 0xF < 0xA && self.value >> 4 < 0xA);
            false
        }
    }

    pub fn value(&self) -> u8 { self.value }
    pub fn set_value(&mut self, value: u8) { self.value = value; assert!(self.value & 0xF < 0xA && self.value >> 4 < 0xA) }
}
