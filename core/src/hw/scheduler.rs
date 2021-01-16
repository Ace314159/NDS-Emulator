use std::cmp::{PartialEq, Eq, Reverse};
use std::hash::Hash;

use priority_queue::PriorityQueue;

use super::{HW, spu};

impl HW {
    pub fn handle_events(&mut self, arm7_cycles: usize) {
        self.scheduler.cycle += arm7_cycles;
        while let Some(event) = self.scheduler.get_next_event() {
            (event.handler)(self, event.event);
        }
    }

    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::DMA(_, _) => self.on_dma(event),
            Event::StartNextLine => self.start_next_line(event),
            Event::HBlank => self.on_hblank(event),
            Event::VBlank => self.on_vblank(event),
            Event::TimerOverflow(_, _) => self.on_timer_overflow(event),
            Event::ROMWordTransfered => self.on_rom_word_transfered(event),
            Event::ROMBlockEnded(_) => self.on_rom_block_ended(event),
            Event::GenerateAudioSample => self.spu.generate_sample(&mut self.scheduler),
            Event::StepAudioChannel(channel_spec) => match channel_spec {
                // TODO: Figure out how to avoid code duplication
                // TODO: Use SPU FIFO
                spu::ChannelSpec::Base(num) => {
                    let format = self.spu.base_channels[num].format();
                    match format {
                        spu::Format::PCM8 => {
                            let (addr, reset) = self.spu.base_channels[num].next_addr_pcm::<u8>();
                            self.spu.base_channels[num].schedule(&mut self.scheduler, reset);
                            let sample = self.arm7_read::<u8>(addr);
                            self.spu.base_channels[num].set_sample(sample);
                        },
                        spu::Format::PCM16 => {
                            let (addr, reset) = self.spu.base_channels[num].next_addr_pcm::<u16>();
                            self.spu.base_channels[num].schedule(&mut self.scheduler, reset);
                            let sample = self.arm7_read::<u16>(addr);
                            self.spu.base_channels[num].set_sample(sample);
                        },
                        spu::Format::ADPCM => {
                            let reset = if let Some(addr) = self.spu.base_channels[num].initial_adpcm_addr() {
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
                        },
                        _ => todo!(),
                    }
                    if let Some((addr, capture_i, use_pcm8)) = self.spu.capture_addr(num) {
                        if use_pcm8 {
                            let value = self.spu.capture_data(capture_i);
                            self.arm7_write::<u8>(addr, value);
                        } else {
                            let value = self.spu.capture_data(capture_i);
                            self.arm7_write::<u16>(addr, value);
                        }
                    }
                },
                spu::ChannelSpec::PSG(num) => {
                    let format = self.spu.psg_channels[num].format();
                    match format {
                        spu::Format::PCM8 => {
                            let (addr, reset) = self.spu.psg_channels[num].next_addr_pcm::<u8>();
                            self.spu.psg_channels[num].schedule(&mut self.scheduler, reset);
                            let sample = self.arm7_read::<u8>(addr);
                            self.spu.psg_channels[num].set_sample(sample);
                        },
                        spu::Format::PCM16 => {
                            let (addr, reset) = self.spu.psg_channels[num].next_addr_pcm::<u16>();
                            self.spu.psg_channels[num].schedule(&mut self.scheduler, reset);
                            let sample = self.arm7_read::<u16>(addr);
                            self.spu.psg_channels[num].set_sample(sample);
                        },
                        spu::Format::ADPCM => {
                            let reset = if let Some(addr) = self.spu.psg_channels[num].initial_adpcm_addr() {
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
                        },
                        _ => todo!(),
                    }
                },
                spu::ChannelSpec::Noise(num) => {
                    let format = self.spu.noise_channels[num].format();
                    match format {
                        spu::Format::PCM8 => {
                            let (addr, reset) = self.spu.noise_channels[num].next_addr_pcm::<u8>();
                            self.spu.noise_channels[num].schedule(&mut self.scheduler, reset);
                            let sample = self.arm7_read::<u8>(addr);
                            self.spu.noise_channels[num].set_sample(sample);
                        },
                        spu::Format::PCM16 => {
                            let (addr, reset) = self.spu.noise_channels[num].next_addr_pcm::<u16>();
                            self.spu.noise_channels[num].schedule(&mut self.scheduler, reset);
                            let sample = self.arm7_read::<u16>(addr);
                            self.spu.noise_channels[num].set_sample(sample);
                        },
                        spu::Format::ADPCM => {
                            let reset = if let Some(addr) = self.spu.noise_channels[num].initial_adpcm_addr() {
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
                        },
                        _ => todo!(),
                    }
                },
            },
            Event::ResetAudioChannel(channel_spec) => match channel_spec {
                spu::ChannelSpec::Base(num) => self.spu.base_channels[num].reset_sample(),
                spu::ChannelSpec::PSG(num) => self.spu.psg_channels[num].reset_sample(),
                spu::ChannelSpec::Noise(num) => self.spu.noise_channels[num].reset_sample(),
            },
        }
    }
}

pub struct Scheduler {
    pub cycle: usize,
    event_queue: PriorityQueue<EventWrapper, Reverse<usize>>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        let queue = PriorityQueue::new();
        Scheduler {
            cycle: 0,
            event_queue: queue,
        }
    }

    fn get_next_event(&mut self) -> Option<EventWrapper> {
        // There should always be at least one event in the queue
        let (_event_type, cycle) = self.event_queue.peek().unwrap();
        if Reverse(self.cycle) <= *cycle {
            Some(self.event_queue.pop().unwrap().0)
        } else { None }
    }

    pub fn schedule(&mut self, event: Event, delay: usize) {
        let wrapper = EventWrapper::new(event, HW::handle_event);
        self.event_queue.push(wrapper, Reverse(self.cycle + delay));
    }

    pub fn run_now(&mut self, event: Event) {
        self.schedule(event, 0);
    }

    pub fn remove(&mut self, event: Event) {
        let wrapper = EventWrapper::new(event, HW::handle_event);
        self.event_queue.remove(&wrapper);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    DMA(bool, usize),
    StartNextLine,
    HBlank,
    VBlank,
    TimerOverflow(bool, usize),
    ROMWordTransfered,
    ROMBlockEnded(bool),
    GenerateAudioSample,
    StepAudioChannel(spu::ChannelSpec),
    ResetAudioChannel(spu::ChannelSpec),
}

struct EventWrapper {
    event: Event,
    handler: fn(&mut HW, Event),
}

impl EventWrapper {
    pub fn new(event: Event, handler: fn(&mut HW, Event)) -> Self {
        EventWrapper {
            event,
            handler,
        }
    }
}

impl PartialEq for EventWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.event.eq(&other.event)
    }
}

impl Eq for EventWrapper {}

impl Hash for EventWrapper {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.event.hash(state);
    }
}
