mod registers;
mod audio;

pub use registers::Format;

use super::{Event, Scheduler};
use crate::hw::mmu::IORegister;

use registers::*;
use audio::Audio;

pub struct SPU {
    cnt: SoundControl,
    // Sound Generation
    audio: Audio,
    clocks_per_sample: usize,
    // Channels
    pub base_channels: [Channel<BaseChannel>; 8],
    pub psg_channels: [Channel<PSGChannel>; 6],
    pub noise_channels: [Channel<NoiseChannel>; 2],
}

macro_rules! create_channels {
    ($type:ident, $spec:ident, $( $num:expr ), *) => {
        [
            $(
                Channel::<$type>::new(ChannelSpec::$spec($num)),
            )*
        ]
    };
}

impl SPU {
    pub fn new(scheduler: &mut Scheduler) -> Self {
        let audio = Audio::new();
        // TODO: Sample at 32.768 kHz and resample to device sample rate
        let clocks_per_sample = crate::nds::NDS::CLOCK_RATE / audio.sample_rate();
        scheduler.schedule(Event::GenerateAudioSample, clocks_per_sample);
        SPU {
            cnt: SoundControl::new(),
            // Sound Generation
            audio,
            clocks_per_sample,
            // Channels
            base_channels: create_channels!(BaseChannel, Base, 0, 1, 2, 3, 4, 5, 6, 7),
            psg_channels: create_channels!(PSGChannel, PSG, 0, 1, 2, 3, 4, 5),
            noise_channels: create_channels!(NoiseChannel, Noise, 0, 1),
        }
    }

    pub fn generate_sample(&mut self, scheduler: &mut Scheduler) {
        scheduler.schedule(Event::GenerateAudioSample, self.clocks_per_sample);

        let mut mixer = (0, 0);
        for i in (0..1).chain(2..3).chain(4..self.base_channels.len()) { self.base_channels[i].generate_sample(&mut mixer) }
        for channel in self.psg_channels.iter() { channel.generate_sample(&mut mixer) }
        for channel in self.noise_channels.iter() { channel.generate_sample(&mut mixer) }
        let (mut ch1, mut ch3) = ((0, 0), (0, 0));
        self.base_channels[1].generate_sample(&mut ch1);
        self.base_channels[3].generate_sample(&mut ch3);
        if self.cnt.output_1 { mixer.0 += ch1.0; mixer.1 += ch1.1 }
        if self.cnt.output_3 { mixer.0 += ch3.0; mixer.1 += ch3.1 }
        let left_sample = match self.cnt.left_output {
            ChannelOutput::Mixer => mixer.0,
            ChannelOutput::Ch1 => ch1.0,
            ChannelOutput::Ch3 => ch3.0,
            ChannelOutput::Ch1Ch3 => todo!(),
        } >> 16;
        let right_sample = match self.cnt.right_output {
            ChannelOutput::Mixer => mixer.1,
            ChannelOutput::Ch1 => ch1.1,
            ChannelOutput::Ch3 => ch3.1,
            ChannelOutput::Ch1Ch3 => todo!(),
        } >> 16;
        let final_sample = (
            ((left_sample * self.cnt.master_volume()) >> 7) as i16,
            ((right_sample * self.cnt.master_volume()) >> 7) as i16,
        );
        self.audio.push_sample(
            cpal::Sample::from::<i16>(&final_sample.0),
            cpal::Sample::from::<i16>(&final_sample.1),
        );
    }

    pub fn read_channels(&self, addr: usize) -> u8 {
        let addr = addr as usize;
        let channel = (addr >> 4) & 0xF;
        let byte = addr & 0xF;
        match channel {
            0x0 ..= 0x7 => self.base_channels[channel].read(byte),
            0x8 ..= 0xD => self.psg_channels[channel - 8].read(byte),
            0xE ..= 0xF => self.noise_channels[channel - 14].read(byte),
            _ => unreachable!(),
        }
    }

    pub fn write_channels(&mut self, scheduler: &mut Scheduler, addr: usize, value: u8) {
        let addr = addr as usize;
        let channel = (addr >> 4) & 0xF;
        let byte = addr & 0xF;
        match channel {
            0x0 ..= 0x7 => self.base_channels[channel].write(scheduler, byte, value),
            0x8 ..= 0xD => self.psg_channels[channel - 0x8].write(scheduler, byte, value),
            0xE ..= 0xF => self.noise_channels[channel - 0xE].write(scheduler, byte, value),
            _ => unreachable!(),
        }
    }
}

impl IORegister for SPU {
    fn read(&self, addr: usize) -> u8 {
        match addr {
            0x400 ..= 0x4FF => self.read_channels(addr),
            0x500 ..= 0x503 => self.cnt.read(addr & 0x3),
            _ => { warn!("Ignoring SPU Register Read at 0x04000{:03X}", addr); 0 }
        }
    }

    fn write(&mut self, scheduler: &mut Scheduler, addr: usize, value: u8) {
        match addr {
            0x400 ..= 0x4FF => self.write_channels(scheduler, addr & 0xFF, value),
            0x500 ..= 0x503 => self.cnt.write(scheduler, addr & 0x3, value),
            _ => warn!("Ignoring SPU Register Write at 0x04000{:03X}", addr)
        }
    }
}

pub struct Channel<T: ChannelType> {
    // Registers
    cnt: ChannelControl<T>,
    src_addr: u32,
    timer_val: u16,
    loop_start: u16,
    len: u32,
    // Sample Generation
    spec: ChannelSpec,
    addr: u32,
    num_bytes_left: usize,
    sample: i16,
    loop_started: bool,
}

impl<T: ChannelType> IORegister for Channel<T> {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0x0 ..= 0x3 => self.cnt.read(byte & 0x3),
            0x4 ..= 0x7 => { warn!("Reading from Write-Only SPU Register: Src Addr"); 0 },
            0x8 ..= 0x9 => { warn!("Reading from Write-Only SPU Register: Timer"); 0 },
            0xA ..= 0xB => { warn!("Reading from Write-Only SPU Register: Loop Start"); 0 },
            0xC ..= 0xF => { warn!("Reading from Write-Only SPU Register: Len"); 0 },
            _ => unreachable!(),
        }
    }

    fn write(&mut self, scheduler: &mut super::scheduler::Scheduler, byte: usize, value: u8) {
        let shift16 = 8 * (byte & 0x1);
        let shift32 = 8 * (byte & 0x3);
        let mask16 = 0xFF << shift16;
        let mask32 = 0xFF << shift32;
        let value16 = (value as u16) << shift16;
        let value32 = (value as u32) << shift32;
        match byte {
            0x0 ..= 0x2 => self.cnt.write(scheduler, byte & 0x3, value),
            0x3 => {
                let prev_busy = self.cnt.busy;
                self.cnt.write(scheduler, byte & 0x3, value);
                if !prev_busy && self.cnt.busy {
                    self.loop_started = false;
                    self.schedule(scheduler, false);
                } else if !self.cnt.busy {
                    scheduler.remove(Event::StepAudioChannel(self.spec));
                }
            }
            0x4 ..= 0x7 => {
                self.src_addr = (self.src_addr & !mask32 | value32) & 0x3FF_FFFF;
                self.addr = self.src_addr;
                // TODO: Behavior when channel has already started
            },
            0x8 ..= 0x9 => {
                self.timer_val = self.timer_val & !mask16 | value16;
                self.schedule(scheduler, false);
            },
            0xA ..= 0xB => self.loop_start = self.loop_start & !mask16 | value16,
            0xC ..= 0xF => {
                self.len = (self.len & !mask32 | value32) & 0x3F_FFFF;
                self.num_bytes_left = self.len as usize * 4;
            },
            _ => unreachable!(),
        }
    }
}

impl<T: ChannelType> Channel<T> {
    pub fn new(spec: ChannelSpec) -> Self {
        Channel {
            // Registers
            cnt: ChannelControl::new(),
            src_addr: 0,
            timer_val: 0,
            loop_start: 0,
            len: 0,
            // Sound Generation
            spec,
            addr: 0,
            num_bytes_left: 0,
            sample: 0,
            loop_started: false,
        }
    }

    fn generate_sample(&self, sample: &mut (i32, i32)) {
        // TODO: Use volume and panning
        sample.0 += ((self.sample as i32) >> self.cnt.volume_shift()) *
            self.cnt.volume_factor() *
            (128 - self.cnt.pan_factor());
        sample.1 += ((self.sample as i32) >> self.cnt.volume_shift()) *
            self.cnt.volume_factor() *
            (self.cnt.pan_factor());
    }

    pub fn next_addr<M: super::MemoryValue>(&mut self) -> (u32, bool) {
        assert!(self.num_bytes_left > 0);
        let return_addr = self.addr;
        self.addr += std::mem::size_of::<M>() as u32;
        self.num_bytes_left -= std::mem::size_of::<M>();
        let reset = if !self.loop_started && self.addr - self.src_addr == self.loop_start as u32 {
            self.loop_started = true;
            self.addr = match self.cnt.repeat_mode {
                RepeatMode::Manual => self.addr,
                RepeatMode::Loop | RepeatMode::OneShot => self.src_addr,
            };
            false
        } else if self.num_bytes_left == 0 {
            // TODO: Verify out timing of busy bit for other modes
            let (reset, new_busy) = match self.cnt.repeat_mode {
                RepeatMode::Manual => (true, true),
                RepeatMode::Loop => {
                    self.addr = self.src_addr;
                    self.num_bytes_left = self.len as usize * 4;
                    (false, true)
                },
                RepeatMode::OneShot => (true, false),
            };
            self.cnt.busy = new_busy;
            reset
        } else { false };
        (return_addr, reset)
    }

    pub fn reset_sample(&mut self) {
        self.sample = 0;
        self.cnt.busy = false;
    }

    pub fn set_sample<M: super::MemoryValue>(&mut self, sample: M) {
        let sample = num_traits::cast::<M, u16>(sample).unwrap();
        self.sample = if std::mem::size_of::<M>() == 1 { sample << 8 } else { sample } as i16;
    }

    pub fn format(&self) -> Format {
        self.cnt.format
    }

    pub fn schedule(&mut self, scheduler: &mut Scheduler, reset: bool) {
        if self.timer_val != 0 {
            if reset {
                scheduler.schedule(Event::ResetAudioChannel(self.spec), (-(self.timer_val as i16) as u16) as usize);
            } else {
                scheduler.schedule(Event::StepAudioChannel(self.spec), (-(self.timer_val as i16) as u16) as usize);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChannelSpec {
    Base(usize),
    PSG(usize),
    Noise(usize),
}

pub trait ChannelType {
    fn supports_psg() -> bool;
    fn supports_noise() -> bool;
}
#[derive(Clone, Copy)]
pub struct BaseChannel {}
#[derive(Clone, Copy)]
pub struct PSGChannel {}
#[derive(Clone, Copy)]
pub struct NoiseChannel {}

impl ChannelType for BaseChannel {
    fn supports_psg() -> bool { return false }
    fn supports_noise() -> bool { return false }
}

impl ChannelType for PSGChannel {
    fn supports_psg() -> bool { return true }
    fn supports_noise() -> bool { return false }
}

impl ChannelType for NoiseChannel {
    fn supports_psg() -> bool { return false }
    fn supports_noise() -> bool { return true }
}
