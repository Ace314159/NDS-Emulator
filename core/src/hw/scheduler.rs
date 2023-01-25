use std::cmp::{Eq, PartialEq, Reverse};
use std::hash::Hash;

use priority_queue::PriorityQueue;

use super::{spu, HW};

type EventHandler = fn(&mut HW, Event);

impl HW {
    pub fn handle_events(&mut self, new_cycle: usize) {
        assert!(self.scheduler.cycle <= new_cycle);
        self.scheduler.cycle = new_cycle;
        while let Some(wrapper) = self.scheduler.get_next_event() {
            (wrapper.handler)(self, wrapper.event);
        }
    }

    pub fn clock_until_event(&mut self) {
        let (_, Reverse(cycle)) = self.scheduler.event_queue.peek().unwrap();
        if self.scheduler.cycle > *cycle {
            return;
        }
        let (wrapper, Reverse(cycle)) = self.scheduler.event_queue.pop().unwrap();
        self.scheduler.cycle = cycle;
        (wrapper.handler)(self, wrapper.event);
    }

    pub fn cycle(&self) -> usize {
        self.scheduler.cycle
    }

    pub fn cycle_at_next_event(&self) -> usize {
        let (_wrapper, Reverse(cycle)) = self.scheduler.event_queue.peek().unwrap();
        if self.scheduler.cycle > *cycle {
            self.scheduler.cycle
        } else {
            *cycle
        }
    }

    fn dummy_handler(&mut self, _event: Event) {
        unreachable!()
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
        let (_event_type, Reverse(cycle)) = self.event_queue.peek().unwrap();
        if self.cycle >= *cycle {
            Some(self.event_queue.pop().unwrap().0)
        } else {
            None
        }
    }

    pub fn schedule(&mut self, event: Event, handler: EventHandler, delay: usize) {
        let wrapper = EventWrapper::new(event, handler);
        self.event_queue.push(wrapper, Reverse(self.cycle + delay));
    }

    pub fn run_now(&mut self, event: Event, handler: EventHandler) {
        self.schedule(event, handler, 0);
    }

    pub fn remove(&mut self, event: Event) {
        let wrapper = EventWrapper::new(event, HW::dummy_handler);
        self.event_queue.remove(&wrapper);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    DMA(bool, usize),
    StartNextLine,
    HBlank,
    VBlank,
    CheckGeometryCommandFIFO,
    TimerOverflow(bool, usize),
    ROMWordTransfered(bool),
    ROMBlockEnded(bool),
    GenerateAudioSample,
    StepAudioChannel(spu::ChannelSpec),
    ResetAudioChannel(spu::ChannelSpec),
}

struct EventWrapper {
    event: Event,
    handler: EventHandler,
}

impl EventWrapper {
    pub fn new(event: Event, handler: EventHandler) -> Self {
        EventWrapper { event, handler }
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
