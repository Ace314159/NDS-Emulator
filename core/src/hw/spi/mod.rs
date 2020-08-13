mod tsc;

use super::{mmu::IORegister, Scheduler};
use crate::hw::cartridge::{Backup, Flash};

pub struct SPI {
    cnt: CNT,
    firmware: Flash,
}

impl SPI {
    pub fn new(firmware: Vec<u8>) -> Self {
        SPI {
            cnt: CNT::new(),
            firmware: Flash::new_firmware(firmware),
        }
    }

    pub fn read_cnt(&self, byte: usize) -> u8 { if self.cnt.enable { self.cnt.read(byte) } else { 0 } }
    pub fn read_data(&self) -> u8 {
        match self.cnt.device {
            Device::Firmware => self.firmware.read(),
            _ => 0,
        }
    }
    
    pub fn write_cnt(&mut self, scheduler: &mut Scheduler, byte: usize, value: u8) {
        let prev_enable = self.cnt.enable;
        let prev_device = self.cnt.device;
        self.cnt.write(scheduler, byte, value);
        if prev_enable && !self.cnt.enable {
            // Disabling requires device to be reset for libnds to work
            match prev_device {
                Device::Firmware => self.firmware.deselect(),
                _ => (),
            }
        }
    }

    pub fn write_data(&mut self, value: u8) {
        if !self.cnt.enable { return }
        match self.cnt.device {
            Device::Firmware => self.firmware.write(self.cnt.hold, value),
            _ => (),
        }
    }
}

pub struct CNT {
    baudrate: u8,
    busy: bool,
    device: Device,
    transfer16: bool,
    hold: bool,
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
            hold: false,
            irq: false,
            enable: false,
        }
    }
}

impl IORegister for CNT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.busy as u8) << 7 | self.baudrate,
            1 => (self.enable as u8) << 7 | (self.irq as u8) << 6 | (self.hold as u8) << 3 |
                (self.transfer16 as u8) << 2 | (self.device as u8),
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => {
                // TODO: Set busy flag properly
                self.baudrate = value & 0x3;
            },
            1 => {
                self.enable = value >> 7 & 0x1 != 0;
                self.irq = value >> 6 & 0x1 != 0;
                self.hold = value >> 3 & 0x1 != 0;
                self.transfer16 = value >> 2 & 0x1 != 0;
                assert!(!self.transfer16);
                self.device = Device::from_bits(value & 0x3);
            },
            _ => unreachable!(),
        }
    }
    
}

#[derive(Clone, Copy, Debug)]
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
