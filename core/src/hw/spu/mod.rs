mod audio;
mod registers;

use super::{
    mem::IORegister,
    scheduler::{Event, Scheduler},
    HW,
};

use audio::Audio;
use registers::*;

pub struct SPU {
    cnt: SoundControl,
    sound_bias: u16,
    captures: [Capture; 2],
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
    pub const ADPCM_INDEX_TABLE: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];
    pub const ADPCM_TABLE: [u16; 89] = [
        0x0007, 0x0008, 0x0009, 0x000A, 0x000B, 0x000C, 0x000D, 0x000E, 0x0010, 0x0011, 0x0013,
        0x0015, 0x0017, 0x0019, 0x001C, 0x001F, 0x0022, 0x0025, 0x0029, 0x002D, 0x0032, 0x0037,
        0x003C, 0x0042, 0x0049, 0x0050, 0x0058, 0x0061, 0x006B, 0x0076, 0x0082, 0x008F, 0x009D,
        0x00AD, 0x00BE, 0x00D1, 0x00E6, 0x00FD, 0x0117, 0x0133, 0x0151, 0x0173, 0x0198, 0x01C1,
        0x01EE, 0x0220, 0x0256, 0x0292, 0x02D4, 0x031C, 0x036C, 0x03C3, 0x0424, 0x048E, 0x0502,
        0x0583, 0x0610, 0x06AB, 0x0756, 0x0812, 0x08E0, 0x09C3, 0x0ABD, 0x0BD0, 0x0CFF, 0x0E4C,
        0x0FBA, 0x114C, 0x1307, 0x14EE, 0x1706, 0x1954, 0x1BDC, 0x1EA5, 0x21B6, 0x2515, 0x28CA,
        0x2CDF, 0x315B, 0x364B, 0x3BB9, 0x41B2, 0x4844, 0x4F7E, 0x5771, 0x602F, 0x69CE, 0x7462,
        0x7FFF,
    ];

    pub fn new(scheduler: &mut Scheduler) -> Self {
        let audio = Audio::new();
        // TODO: Sample at 32.768 kHz and resample to device sample rate
        let clocks_per_sample = crate::nds::NDS::CLOCK_RATE / audio.sample_rate();
        scheduler.schedule(
            Event::GenerateAudioSample,
            HW::generate_audio_sample,
            clocks_per_sample,
        );
        SPU {
            cnt: SoundControl::new(),
            sound_bias: 0,
            captures: [Capture::new(), Capture::new()],
            // Sound Generation
            audio,
            clocks_per_sample,
            // Channels
            base_channels: create_channels!(BaseChannel, Base, 0, 1, 2, 3, 4, 5, 6, 7),
            psg_channels: create_channels!(PSGChannel, PSG, 0, 1, 2, 3, 4, 5),
            noise_channels: create_channels!(NoiseChannel, Noise, 0, 1),
        }
    }

    fn generate_mixer(&self) -> ((i32, i32), (i32, i32), (i32, i32)) {
        let mut mixer = (0, 0);
        for i in (0..1).chain(2..3).chain(4..self.base_channels.len()) {
            self.base_channels[i].generate_sample(&mut mixer)
        }
        for channel in self.psg_channels.iter() {
            channel.generate_sample(&mut mixer)
        }
        for channel in self.noise_channels.iter() {
            channel.generate_sample(&mut mixer)
        }
        let (mut ch1, mut ch3) = ((0, 0), (0, 0));
        self.base_channels[1].generate_sample(&mut ch1);
        self.base_channels[3].generate_sample(&mut ch3);
        if self.cnt.output_1 {
            mixer.0 += ch1.0;
            mixer.1 += ch1.1
        }
        if self.cnt.output_3 {
            mixer.0 += ch3.0;
            mixer.1 += ch3.1
        }
        (mixer, ch1, ch3)
    }

    pub fn generate_sample(&mut self) {
        let (mixer, ch1, ch3) = self.generate_mixer();
        let left_sample = match self.cnt.left_output {
            ChannelOutput::Mixer => mixer.0,
            ChannelOutput::Ch1 => ch1.0,
            ChannelOutput::Ch3 => ch3.0,
            ChannelOutput::Ch1Ch3 => ch1.0 + ch3.0,
        } >> 16;
        let right_sample = match self.cnt.right_output {
            ChannelOutput::Mixer => mixer.1,
            ChannelOutput::Ch1 => ch1.1,
            ChannelOutput::Ch3 => ch3.1,
            ChannelOutput::Ch1Ch3 => ch1.0 + ch3.0,
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

    pub fn capture_addr(&mut self, num: usize) -> Option<(u32, usize, bool)> {
        let capture_i = match num {
            1 => 0,
            3 => 1,
            _ => return None,
        };
        let capture = &mut self.captures[capture_i];
        if capture.num_bytes_left == 0 || !capture.cnt.busy {
            return None;
        }
        if capture.cnt.use_pcm8 {
            Some((capture.next_addr::<u8>(), capture_i, true))
        } else {
            Some((capture.next_addr::<u16>(), capture_i, false))
        }
    }

    pub fn capture_data<T: super::MemoryValue>(&self, capture_i: usize) -> T {
        let capture_value = if self.captures[capture_i].cnt.use_channel {
            // TODO: Implement bugged behavior
            todo!()
        } else {
            let (mixer, _, _) = self.generate_mixer();
            let mixer_value = (if capture_i == 0 { mixer.0 } else { mixer.1 } >> 16) as u16;
            if std::mem::size_of::<T>() == 1 {
                mixer_value >> 8
            } else {
                mixer_value
            }
        };
        if self.captures[capture_i].cnt.add {
            // TODO: Implement adding channel
            todo!()
        } else {
            num_traits::cast(capture_value).unwrap()
        }
    }

    pub fn read_channels(&self, addr: usize) -> u8 {
        let addr = addr as usize;
        let channel = (addr >> 4) & 0xF;
        let byte = addr & 0xF;
        match channel {
            0x0..=0x7 => self.base_channels[channel].read(byte),
            0x8..=0xD => self.psg_channels[channel - 0x8].read(byte),
            0xE..=0xF => self.noise_channels[channel - 0xE].read(byte),
            _ => unreachable!(),
        }
    }

    pub fn write_channels(&mut self, scheduler: &mut Scheduler, addr: usize, value: u8) {
        let addr = addr as usize;
        let channel = (addr >> 4) & 0xF;
        let byte = addr & 0xF;
        match channel {
            0x0..=0x7 => self.base_channels[channel].write(scheduler, byte, value),
            0x8..=0xD => self.psg_channels[channel - 0x8].write(scheduler, byte, value),
            0xE..=0xF => self.noise_channels[channel - 0xE].write(scheduler, byte, value),
            _ => unreachable!(),
        }
    }
}

impl IORegister for SPU {
    fn read(&self, addr: usize) -> u8 {
        match addr {
            0x400..=0x4FF => self.read_channels(addr),
            0x500..=0x503 => self.cnt.read(addr & 0x3),
            0x504..=0x507 => HW::read_byte_from_value(&self.sound_bias, addr & 0x3),
            0x508..=0x509 => self.captures[addr & 0x1].cnt.read(),
            0x510..=0x51F => self.captures[addr >> 3 & 0x1].read(addr & 0xF),
            _ => {
                warn!("Ignoring SPU Register Read at 0x04000{:03X}", addr);
                0
            }
        }
    }

    fn write(&mut self, scheduler: &mut Scheduler, addr: usize, value: u8) {
        match addr {
            0x400..=0x4FF => self.write_channels(scheduler, addr & 0xFF, value),
            0x500..=0x503 => self.cnt.write(scheduler, addr & 0x3, value),
            0x504..=0x507 => {
                HW::write_byte_to_value(&mut self.sound_bias, addr & 0x3, value);
                self.sound_bias &= 0x3FF;
            }
            0x508..=0x509 => self.captures[addr & 0x1].write_cnt(value),
            0x510..=0x51F => self.captures[addr >> 3 & 0x1].write(addr & 0x7, value),
            _ => warn!("Ignoring SPU Register Write at 0x04000{:03X}", addr),
        }
    }
}

impl HW {
    fn generate_audio_sample(&mut self, _event: Event) {
        self.scheduler.schedule(
            Event::GenerateAudioSample,
            HW::generate_audio_sample,
            self.spu.clocks_per_sample,
        );
        self.spu.generate_sample();
    }

    fn step_audio_channel(&mut self, event: Event) {
        let channel_spec = match event {
            Event::StepAudioChannel(channel_spec) => channel_spec,
            _ => unreachable!(),
        };
        match channel_spec {
            // TODO: Figure out how to avoid code duplication
            // TODO: Use SPU FIFO
            ChannelSpec::Base(num) => {
                let format = self.spu.base_channels[num].format();
                match format {
                    Format::PCM8 => {
                        let (addr, reset) = self.spu.base_channels[num].next_addr_pcm::<u8>();
                        self.spu.base_channels[num].schedule(&mut self.scheduler, reset);
                        let sample = self.arm7_read::<u8>(addr);
                        self.spu.base_channels[num].set_sample(sample);
                    }
                    Format::PCM16 => {
                        let (addr, reset) = self.spu.base_channels[num].next_addr_pcm::<u16>();
                        self.spu.base_channels[num].schedule(&mut self.scheduler, reset);
                        let sample = self.arm7_read::<u16>(addr);
                        self.spu.base_channels[num].set_sample(sample);
                    }
                    Format::ADPCM => {
                        let reset =
                            if let Some(addr) = self.spu.base_channels[num].initial_adpcm_addr() {
                                let value = self.arm7_read::<u32>(addr);
                                self.spu.base_channels[num].set_initial_adpcm(value);
                                false
                            } else {
                                let (addr, reset) = self.spu.base_channels[num].next_addr_adpcm();
                                let value = self.arm7_read(addr);
                                self.spu.base_channels[num].set_adpcm_data(value);
                                reset
                            };
                        self.spu.base_channels[num].schedule(&mut self.scheduler, reset);
                    }
                    _ => todo!(),
                }
                if let Some((_addr, capture_i, use_pcm8)) = self.spu.capture_addr(num) {
                    if use_pcm8 {
                        let _value: u8 = self.spu.capture_data(capture_i);
                        //self.arm7_write::<u8>(addr, value);
                    } else {
                        let _value: u16 = self.spu.capture_data(capture_i);
                        //self.arm7_write::<u16>(addr, value);
                    }
                }
            }
            ChannelSpec::PSG(num) => {
                let format = self.spu.psg_channels[num].format();
                match format {
                    Format::PCM8 => {
                        let (addr, reset) = self.spu.psg_channels[num].next_addr_pcm::<u8>();
                        self.spu.psg_channels[num].schedule(&mut self.scheduler, reset);
                        let sample = self.arm7_read::<u8>(addr);
                        self.spu.psg_channels[num].set_sample(sample);
                    }
                    Format::PCM16 => {
                        let (addr, reset) = self.spu.psg_channels[num].next_addr_pcm::<u16>();
                        self.spu.psg_channels[num].schedule(&mut self.scheduler, reset);
                        let sample = self.arm7_read::<u16>(addr);
                        self.spu.psg_channels[num].set_sample(sample);
                    }
                    Format::ADPCM => {
                        let reset =
                            if let Some(addr) = self.spu.psg_channels[num].initial_adpcm_addr() {
                                let value = self.arm7_read::<u32>(addr);
                                self.spu.psg_channels[num].set_initial_adpcm(value);
                                false
                            } else {
                                let (addr, reset) = self.spu.psg_channels[num].next_addr_adpcm();
                                let value = self.arm7_read(addr);
                                self.spu.psg_channels[num].set_adpcm_data(value);
                                reset
                            };
                        self.spu.psg_channels[num].schedule(&mut self.scheduler, reset);
                    }
                    _ => todo!(),
                }
            }
            ChannelSpec::Noise(num) => {
                let format = self.spu.noise_channels[num].format();
                match format {
                    Format::PCM8 => {
                        let (addr, reset) = self.spu.noise_channels[num].next_addr_pcm::<u8>();
                        self.spu.noise_channels[num].schedule(&mut self.scheduler, reset);
                        let sample = self.arm7_read::<u8>(addr);
                        self.spu.noise_channels[num].set_sample(sample);
                    }
                    Format::PCM16 => {
                        let (addr, reset) = self.spu.noise_channels[num].next_addr_pcm::<u16>();
                        self.spu.noise_channels[num].schedule(&mut self.scheduler, reset);
                        let sample = self.arm7_read::<u16>(addr);
                        self.spu.noise_channels[num].set_sample(sample);
                    }
                    Format::ADPCM => {
                        let reset =
                            if let Some(addr) = self.spu.noise_channels[num].initial_adpcm_addr() {
                                let value = self.arm7_read::<u32>(addr);
                                self.spu.noise_channels[num].set_initial_adpcm(value);
                                false
                            } else {
                                let (addr, reset) = self.spu.noise_channels[num].next_addr_adpcm();
                                let value = self.arm7_read(addr);
                                self.spu.noise_channels[num].set_adpcm_data(value);
                                reset
                            };
                        self.spu.noise_channels[num].schedule(&mut self.scheduler, reset);
                    }
                    _ => todo!(),
                }
            }
        }
    }

    fn reset_audio_channel(&mut self, event: Event) {
        let channel_spec = match event {
            Event::ResetAudioChannel(channel_spec) => channel_spec,
            _ => unreachable!(),
        };
        match channel_spec {
            ChannelSpec::Base(num) => self.spu.base_channels[num].reset_sample(),
            ChannelSpec::PSG(num) => self.spu.psg_channels[num].reset_sample(),
            ChannelSpec::Noise(num) => self.spu.noise_channels[num].reset_sample(),
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
    // ADPCM
    adpcm_in_header: bool,
    adpcm_low_nibble: bool,
    adpcm_index: i32,
    adpcm_value: i16,
    initial_adpcm_index: i32,
    initial_adpcm_value: i16,
}

impl<T: ChannelType> IORegister for Channel<T> {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0x0..=0x3 => self.cnt.read(byte & 0x3),
            0x4..=0x7 => {
                warn!("Reading from Write-Only SPU Register: Src Addr");
                0
            }
            0x8..=0x9 => {
                warn!("Reading from Write-Only SPU Register: Timer");
                0
            }
            0xA..=0xB => {
                warn!("Reading from Write-Only SPU Register: Loop Start");
                0
            }
            0xC..=0xF => {
                warn!("Reading from Write-Only SPU Register: Len");
                0
            }
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
        // TODO: Fix inaccurate scheduling timing for maxmod interpolated mode
        match byte {
            0x0..=0x2 => self.cnt.write(scheduler, byte & 0x3, value),
            0x3 => {
                let prev_busy = self.cnt.busy;
                self.cnt.write(scheduler, byte & 0x3, value);
                if !prev_busy && self.cnt.busy {
                    self.adpcm_in_header = true;
                    self.adpcm_low_nibble = true;
                    self.schedule(scheduler, false);
                } else if !self.cnt.busy {
                    scheduler.remove(Event::StepAudioChannel(self.spec));
                }
            }
            0x4..=0x7 => {
                self.src_addr = (self.src_addr & !mask32 | value32) & 0x3FF_FFFF;
                self.addr = self.src_addr;
                // TODO: Behavior when channel has already started
            }
            0x8..=0x9 => {
                self.timer_val = self.timer_val & !mask16 | value16;
                if self.cnt.busy {
                    self.schedule(scheduler, false)
                }
            }
            0xA..=0xB => {
                self.loop_start = self.loop_start & !mask16 | value16;
                self.num_bytes_left = (self.loop_start as usize + self.len as usize) * 4;
                if self.cnt.busy {
                    self.schedule(scheduler, false)
                }
            }
            0xC..=0xF => {
                self.len = (self.len & !mask32 | value32) & 0x3F_FFFF;
                self.num_bytes_left = (self.loop_start as usize + self.len as usize) * 4;
                if self.cnt.busy {
                    self.schedule(scheduler, false)
                }
            }
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
            // ADPCM
            adpcm_in_header: true,
            adpcm_low_nibble: true,
            adpcm_index: 0,
            adpcm_value: 0,
            initial_adpcm_index: 0,
            initial_adpcm_value: 0,
        }
    }

    fn generate_sample(&self, sample: &mut (i32, i32)) {
        // TODO: Use volume and panning
        sample.0 += ((self.sample as i32) >> self.cnt.volume_shift())
            * self.cnt.volume_factor()
            * (128 - self.cnt.pan_factor());
        sample.1 += ((self.sample as i32) >> self.cnt.volume_shift())
            * self.cnt.volume_factor()
            * (self.cnt.pan_factor());
    }

    pub fn next_addr_pcm<M: super::MemoryValue>(&mut self) -> (u32, bool) {
        assert!(self.num_bytes_left > 0);
        let return_addr = self.addr;
        self.addr += std::mem::size_of::<M>() as u32;
        self.num_bytes_left -= std::mem::size_of::<M>();
        let reset = if self.num_bytes_left == 0 {
            self.handle_end()
        } else {
            false
        };
        (return_addr, reset)
    }

    fn handle_end(&mut self) -> bool {
        // TODO: Verify out timing of busy bit for other modes
        let (reset, new_busy) = match self.cnt.repeat_mode {
            RepeatMode::Manual => (true, true),
            RepeatMode::Loop => {
                self.addr = self.src_addr + self.loop_start as u32 * 4;
                self.adpcm_low_nibble = true;
                self.num_bytes_left = self.len as usize * 4;
                (false, true)
            }
            RepeatMode::OneShot => (true, false),
        };
        self.cnt.busy = new_busy;
        reset
    }

    pub fn reset_sample(&mut self) {
        self.sample = 0;
        self.cnt.busy = false;
    }

    pub fn set_sample<M: super::MemoryValue>(&mut self, sample: M) {
        let sample = num_traits::cast::<M, u16>(sample).unwrap();
        self.sample = if std::mem::size_of::<M>() == 1 {
            sample << 8
        } else {
            sample
        } as i16;
    }

    pub fn initial_adpcm_addr(&mut self) -> Option<u32> {
        if self.adpcm_in_header {
            assert_eq!(self.src_addr, self.addr);
            self.adpcm_in_header = false;
            let return_addr = self.addr;
            self.addr += std::mem::size_of::<u32>() as u32;
            self.num_bytes_left -= std::mem::size_of::<u32>();
            Some(return_addr)
        } else {
            None
        }
    }

    pub fn next_addr_adpcm(&mut self) -> (u32, bool) {
        assert!(self.num_bytes_left > 0);
        let return_addr = self.addr;
        let reset = if self.adpcm_low_nibble {
            false
        } else {
            self.addr += 1;
            self.num_bytes_left -= 1;
            if self.num_bytes_left == 0 {
                self.handle_end()
            } else {
                false
            }
        };
        (return_addr, reset)
    }

    pub fn set_adpcm_data(&mut self, value: u8) {
        let data = if self.adpcm_low_nibble {
            value & 0xF
        } else {
            value >> 4 & 0xF
        };
        self.adpcm_low_nibble = !self.adpcm_low_nibble;
        let table_val = SPU::ADPCM_TABLE[self.adpcm_index as usize];
        let mut diff = table_val / 8;
        if data & 0x1 != 0 {
            diff += table_val / 4
        }
        if data & 0x2 != 0 {
            diff += table_val / 2
        }
        if data & 0x4 != 0 {
            diff += table_val / 1
        }
        if data & 0x8 == 0 {
            self.adpcm_value = self.adpcm_value.saturating_add(diff as i16);
        } else {
            self.adpcm_value = self.adpcm_value.saturating_sub(diff as i16);
        }
        self.adpcm_index += SPU::ADPCM_INDEX_TABLE[data as usize & 0x7];
        self.adpcm_index = self.adpcm_index.clamp(0, 88);

        self.sample = self.adpcm_value as i16;
    }

    pub fn set_initial_adpcm(&mut self, value: u32) {
        self.initial_adpcm_index = (value >> 16 & 0x7F).clamp(0, 88) as i32;
        self.initial_adpcm_value = value as u16 as i16;
        self.reset_adpcm();
    }

    pub fn reset_adpcm(&mut self) {
        self.adpcm_index = self.initial_adpcm_index;
        self.adpcm_value = self.initial_adpcm_value;
    }

    pub fn format(&self) -> Format {
        self.cnt.format
    }

    pub fn schedule(&mut self, scheduler: &mut Scheduler, reset: bool) {
        if self.timer_val != 0 && self.len + self.loop_start as u32 != 0 {
            if reset {
                scheduler.schedule(
                    Event::ResetAudioChannel(self.spec),
                    HW::reset_audio_channel,
                    (-(self.timer_val as i16) as u16) as usize,
                );
            } else {
                scheduler.schedule(
                    Event::StepAudioChannel(self.spec),
                    HW::step_audio_channel,
                    (-(self.timer_val as i16) as u16) as usize,
                );
            }
        }
    }
}

struct Capture {
    // Registers
    cnt: CaptureControl,
    dest_addr: u32,
    len: usize,
    // Sound Capturing
    addr: u32,
    num_bytes_left: usize,
}

impl Capture {
    pub fn new() -> Self {
        Capture {
            // Registers
            cnt: CaptureControl::new(),
            dest_addr: 0,
            len: 0,
            // Sound Capturing
            addr: 0,
            num_bytes_left: 0,
        }
    }

    pub fn next_addr<T: super::MemoryValue>(&mut self) -> u32 {
        assert!(self.num_bytes_left > 0);
        self.num_bytes_left -= std::mem::size_of::<T>();
        self.cnt.busy = self.num_bytes_left > 0;
        let return_addr = self.addr;
        self.addr += std::mem::size_of::<T>() as u32;
        return_addr
    }

    pub fn read(&self, byte: usize) -> u8 {
        let shift = (byte & 0x3) * 8;
        match byte {
            0x0..=0x3 => (self.addr >> shift) as u8,
            0x4..=0x7 => {
                warn!("Reading from Write-Only Sound Capture Register: Dest Addr");
                0
            }
            _ => unreachable!(),
        }
    }

    pub fn write_cnt(&mut self, value: u8) {
        let prev_busy = self.cnt.busy;
        self.cnt.write(value);
        if !prev_busy && self.cnt.busy {
            self.num_bytes_left = self.len * 4;
            self.addr = self.dest_addr;
        }
    }

    pub fn write(&mut self, byte: usize, value: u8) {
        let shift = (byte & 0x3) * 8;
        let mask = 0xFF << shift;
        let value = (value as u32) << shift;
        match byte {
            0x0..=0x3 => {
                self.dest_addr = (value & !mask | value) & 0x7FF_FFFF;
                self.addr = self.dest_addr;
            }
            0x4..=0x7 => {
                self.len = (self.len & !(mask as usize) | (value as usize)) as u16 as usize;
                self.num_bytes_left = self.len * 4;
            }
            _ => unreachable!(),
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
    fn supports_psg() -> bool {
        return false;
    }
    fn supports_noise() -> bool {
        return false;
    }
}

impl ChannelType for PSGChannel {
    fn supports_psg() -> bool {
        return true;
    }
    fn supports_noise() -> bool {
        return false;
    }
}

impl ChannelType for NoiseChannel {
    fn supports_psg() -> bool {
        return false;
    }
    fn supports_noise() -> bool {
        return true;
    }
}
