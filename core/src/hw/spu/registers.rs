use std::marker::PhantomData;

use super::{ChannelType, IORegister, Scheduler};

pub struct SoundControl {
    master_volume: u8,
    pub left_output: ChannelOutput,
    pub right_output: ChannelOutput,
    pub output_1: bool,
    pub output_3: bool,
    pub enable: bool,
}

impl IORegister for SoundControl {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => self.master_volume,
            1 => {
                (self.enable as u8) << 7
                    | (self.output_3 as u8) << 5
                    | (self.output_1 as u8) << 4
                    | (self.right_output as u8) << 2
                    | (self.left_output as u8)
            }
            2 | 3 => 0,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => self.master_volume = value & 0x7F,
            1 => {
                self.left_output = ChannelOutput::from(value >> 0 & 0x3);
                self.right_output = ChannelOutput::from(value >> 2 & 0x3);
                self.output_1 = value >> 4 != 0;
                self.output_3 = value >> 5 != 0;
                self.enable = value >> 7 != 0;
            }
            2 | 3 => (),
            _ => unreachable!(),
        }
    }
}

impl SoundControl {
    pub fn new() -> Self {
        SoundControl {
            master_volume: 0,
            left_output: ChannelOutput::Mixer,
            right_output: ChannelOutput::Mixer,
            output_1: false,
            output_3: false,
            enable: false,
        }
    }

    pub fn master_volume(&self) -> i32 {
        if self.master_volume == 127 {
            128
        } else {
            self.master_volume as i32
        }
    }
}

#[derive(Clone, Copy)]
pub enum ChannelOutput {
    Mixer = 0,
    Ch1 = 1,
    Ch3 = 2,
    Ch1Ch3 = 3,
}

impl From<u8> for ChannelOutput {
    fn from(value: u8) -> Self {
        use ChannelOutput::*;
        match value {
            0 => Mixer,
            1 => Ch1,
            2 => Ch3,
            3 => Ch1Ch3,
            _ => unreachable!(),
        }
    }
}

pub struct ChannelControl<T: ChannelType> {
    volume_mul: u8,
    volume_div: u8,
    pub hold: bool,
    panning: u8,
    pub wave_duty: u8,
    pub repeat_mode: RepeatMode,
    pub format: Format,
    pub busy: bool,
    channel_type: PhantomData<T>,
}

impl<T: ChannelType> IORegister for ChannelControl<T> {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => self.volume_mul,
            1 => (self.hold as u8) << 7 | self.volume_div,
            2 => self.panning,
            3 => {
                (self.busy as u8) << 7
                    | (self.format as u8) << 5
                    | (self.repeat_mode as u8) << 3
                    | self.wave_duty
            }
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => self.volume_mul = value & 0x7F,
            1 => {
                self.hold = (value >> 7) & 0x1 != 0;
                self.volume_div = value >> 0 & 0x3;
            }
            2 => self.panning = value & 0x7F,
            3 => {
                self.wave_duty = value & 0x7;
                self.repeat_mode = RepeatMode::from(value >> 3 & 0x3);
                self.format = Format::from(value >> 5 & 0x3);
                self.busy = value >> 7 & 0x1 != 0;
            }
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

    pub fn volume_shift(&self) -> usize {
        [0, 1, 2, 4][self.volume_div as usize]
    }

    pub fn volume_factor(&self) -> i32 {
        if self.volume_mul == 127 {
            128
        } else {
            self.volume_mul as i32
        }
    }

    pub fn pan_factor(&self) -> i32 {
        if self.panning == 127 {
            128
        } else {
            self.panning as i32
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

pub struct CaptureControl {
    pub add: bool,
    pub use_channel: bool,
    pub no_repeat: bool,
    pub use_pcm8: bool,
    pub busy: bool,
}

impl CaptureControl {
    pub fn new() -> Self {
        CaptureControl {
            add: false,
            use_channel: false,
            no_repeat: false,
            use_pcm8: false,
            busy: false,
        }
    }

    pub fn read(&self) -> u8 {
        (self.busy as u8) << 7
            | (self.use_pcm8 as u8) << 3
            | (self.no_repeat as u8) << 2
            | (self.use_channel as u8) << 1
            | (self.add as u8)
    }

    pub fn write(&mut self, value: u8) {
        self.add = value >> 0 & 0x1 != 0;
        self.use_channel = value >> 1 & 0x1 != 0;
        self.no_repeat = value >> 2 & 0x1 != 0;
        self.use_pcm8 = value >> 3 & 0x1 != 0;
        self.busy = value >> 7 & 0x1 != 0;
    }
}
