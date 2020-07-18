mod firmware;

use super::{mmu::IORegister, Scheduler};
use firmware::Firmware;

pub struct SPI {
    cnt: CNT,
    firmware: Firmware,
}

impl SPI {
    pub fn new(firmware: Vec<u8>) -> Self {
        SPI {
            cnt: CNT::new(),
            firmware: Firmware::new(firmware),
        }
    }

    pub fn read_cnt(&self, byte: usize) -> u8 { self.cnt.read(byte) }
    pub fn read_data(&self) -> u8 {
        match self.cnt.device {
            Device::Firmware => self.firmware.read(),
            _ => todo!(),
        }
    }
    
    pub fn write_cnt(&mut self, scheduler: &mut Scheduler, byte: usize, value: u8) {
        self.cnt.write(scheduler, byte, value);
    }

    pub fn write_data(&mut self, value: u8) {
        match self.cnt.device {
            Device::Firmware => self.firmware.write(value),
            _ => todo!(),
        }
    }
}

pub struct CNT {
    baudrate: u8,
    busy: bool,
    device: Device,
    transfer16: bool,
    chipselect_hold: bool,
    irq: bool,
    enable: bool,
}

impl CNT {
    pub fn new() -> Self {
        CNT {
            baudrate: 0,
            busy: false,
            device: Device::Powerman,
            transfer16: false,
            chipselect_hold: false,
            irq: false,
            enable: false,
        }
    }
}

impl IORegister for CNT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.busy as u8) << 7 | self.baudrate,
            1 => (self.enable as u8) << 7 | (self.irq as u8) << 6 | (self.chipselect_hold as u8) << 3 |
                (self.transfer16 as u8) << 2 | (self.device as u8),
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => {
                self.busy = value >> 7 & 0x1 != 0;
                self.baudrate = value & 0x3;
            },
            1 => {
                self.enable = value >> 7 & 0x1 != 0;
                self.irq = value >> 6 & 0x1 != 0;
                self.chipselect_hold = value >> 3 & 0x1 != 0;
                self.transfer16 = value >> 2 & 0x1 != 0;
                assert!(!self.transfer16);
                self.device = Device::from_bits(value & 0x3);
            },
            _ => unreachable!(),
        }
    }
    
}

#[derive(Clone, Copy)]
enum Device {
    Powerman = 0,
    Firmware = 1,
    Touchscreen = 2,
}

impl Device {
    pub fn from_bits(value: u8) -> Self {
        match value {
            0 => Self::Powerman,
            1 => Self::Firmware,
            2 => Self::Touchscreen,
            3 => panic!("Reserved SPI Device!"),
            _ => unreachable!(),
        }
    }
}
