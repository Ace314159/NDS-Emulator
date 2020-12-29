use std::marker::PhantomData;

use super::{ChannelType, IORegister, Scheduler};

pub struct ChannelControl<T: ChannelType> {
    pub volume_mul: u8,
    pub volume_div: u8,
    pub hold: bool,
    pub panning: u8,
    pub wave_duty: u8,
    pub repeat_mode: RepeatMode,
    pub format: Format,
    pub busy: bool,
    pub channel_type: PhantomData<T>,
}

impl<T: ChannelType> IORegister for ChannelControl<T> {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => self.volume_mul,
            1 => (self.hold as u8) << 7 | self.volume_div,
            2 => self.panning,
            3 => (self.busy as u8) << 7 | (self.format as u8) << 5 | (self.repeat_mode as u8) << 3 | self.wave_duty,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => self.volume_mul = value & 0x3F,
            1 => {
                self.hold = (value >> 7) & 0x1 != 0;
                self.volume_div = value >> 0 & 0x3;
            },
            2 => self.panning = value & 0x7F,
            3 => {
                self.wave_duty = value & 0x7;
                self.repeat_mode = RepeatMode::from(value >> 3 & 0x3);
                self.format = Format::from(value >> 5 & 0x3);
                self.busy = value >> 7 & 0x1 != 0;
            },
            _ => unreachable!(),
        }
    }
}

impl<T: ChannelType> ChannelControl<T> {
    pub fn new() -> Self {
        ChannelControl {
            volume_mul: 0,
            volume_div: 0,
            hold: false,
            panning: 0,
            wave_duty: 0,
            repeat_mode: RepeatMode::Manual,
            format: Format::PCM8,
            busy: false,
            channel_type: PhantomData,
        }
    }
}

#[derive(Clone, Copy)]
pub enum RepeatMode {
    Manual = 0,
    Loop = 1,
    OneShot = 2,
}

impl From<u8> for RepeatMode {
    fn from(value: u8) -> Self {
        use RepeatMode::*;
        match value {
            0 => Manual,
            1 => Loop,
            2 => OneShot,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Format {
    PCM8 = 0,
    PCM16 = 1,
    ADPCM = 2,
    Special = 3,
}

impl From<u8> for Format {
    fn from(value: u8) -> Self {
        use Format::*;
        match value {
            0 => PCM8,
            1 => PCM16,
            2 => ADPCM,
            3 => Special,
            _ => unreachable!(),
        }
    }
}
