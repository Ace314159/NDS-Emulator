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
            Event::GenerateAudioSample => self.generate_audio_sample(event),
            Event::StepAudioChannel(_) => self.step_audio_channel(event),
            Event::ResetAudioChannel(_) => self.reset_audio_channel(event),
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
