mod registers;
mod audio;

use super::{Event, Scheduler};
use crate::hw::mmu::IORegister;

use registers::*;
use audio::Audio;

pub struct SPU {
    // Sound Generation
    audio: Audio,
    clocks_per_sample: usize,
    // Channels
    base_channels: [Channel<BaseChannel>; 8],
    psg_channels: [Channel<PSGChannel>; 6],
    noise_channels: [Channel<NoiseChannel>; 2],
}

impl SPU {
    pub fn new(scheduler: &mut Scheduler) -> Self {
        let audio = Audio::new();
        // TODO: Sample at 32.768 kHz and resample to device sample rate
        let clocks_per_sample = crate::nds::NDS::CLOCK_RATE / audio.sample_rate();
        scheduler.schedule(Event::GenerateAudioSample, clocks_per_sample);

        SPU {
            // Sound Generation
            audio,
            clocks_per_sample,
            // Channels
            base_channels: [Channel::<BaseChannel>::new(); 8],
            psg_channels: [Channel::<PSGChannel>::new(); 6],
            noise_channels: [Channel::<NoiseChannel>::new(); 2],
        }
    }

    pub fn generate_sample(&mut self, scheduler: &mut Scheduler) {
        self.audio.push_sample(0.0, 0.0);
        scheduler.schedule(Event::GenerateAudioSample, self.clocks_per_sample);
    }

    pub fn read_channels(&self, addr: u32) -> u8 {
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

    pub fn write_channels(&mut self, scheduler: &mut Scheduler, addr: u32, value: u8) {
        let addr = addr as usize;
        let channel = (addr >> 4) & 0xF;
        let byte = addr & 0xF;
        match channel {
            0x0 ..= 0x7 => self.base_channels[channel].write(scheduler, byte, value),
            0x8 ..= 0xD => self.psg_channels[channel - 8].write(scheduler, byte, value),
            0xE ..= 0xF => self.noise_channels[channel - 14].write(scheduler, byte, value),
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Channel<T: ChannelType> {
    cnt: ChannelControl<T>,
    src_addr: u32,
    timer_val: u16,
    loop_start: u16,
    len: u32,
}

impl<T: ChannelType> IORegister for Channel<T> {
    fn read(&self, byte: usize) -> u8 {
        let shift16 = 8 * (byte & 0x1);
        let shift32 = 8 * (byte & 0x3);
        match byte {
            0x0 ..= 0x3 => self.cnt.read(byte & 0x3),
            0x4 ..= 0x7 => (self.src_addr >> shift32) as u8,
            0x8 ..= 0x9 => (self.timer_val >> shift16) as u8,
            0xA ..= 0xB => (self.loop_start >> shift16) as u8,
            0xC ..= 0xF => (self.len >> shift32) as u8,
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
            0x0 ..= 0x3 => self.cnt.write(scheduler, byte & 0x3, value),
            0x4 ..= 0x7 => self.src_addr = (self.src_addr & !mask32 | value32) & 0x3FF_FFFF,
            0x8 ..= 0x9 => self.timer_val = self.timer_val & !mask16 | value16,
            0xA ..= 0xB => self.loop_start = self.loop_start & !mask16 | value16,
            0xC ..= 0xF => self.len = (self.len & !mask32 | value32) & 0x3F_FFFF,
            _ => unreachable!(),
        }
    }
}

impl<T: ChannelType> Channel<T> {
    pub fn new() -> Self {
        Channel {
            cnt: ChannelControl::new(),
            src_addr: 0,
            timer_val: 0,
            loop_start: 0,
            len: 0,
        }
    }
}
