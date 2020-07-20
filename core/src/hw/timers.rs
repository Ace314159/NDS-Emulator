use super::{Scheduler, Event, EventType, mmu::IORegister};
use super::InterruptRequest;

pub struct Timers {
    pub timers: [Timer; 4],
}

impl Timers {
    pub const PRESCALERS: [usize; 4] = [1, 64, 256, 1024];

    pub fn new(is_nds9: bool) -> Timers {
        Timers {
            timers: [
                Timer::new(is_nds9, 0, InterruptRequest::TIMER0_OVERFLOW),
                Timer::new(is_nds9, 1, InterruptRequest::TIMER1_OVERFLOW),
                Timer::new(is_nds9, 2, InterruptRequest::TIMER2_OVERFLOW),
                Timer::new(is_nds9, 3, InterruptRequest::TIMER3_OVERFLOW),
            ],
        }
    }
}

#[derive(Clone, Copy)]
pub struct Timer {
    is_nds9: bool,
    pub reload: u16,
    pub cnt: TMCNT,
    pub index: usize,
    pub interrupt: InterruptRequest,
    // Counter Calcuation
    // Count-Up Timing
    counter: u16,
    // Regular Timing
    start_cycle: usize,
    time_till_first_clock: usize,
    timer_len: usize,
}

impl Timer {
    pub fn new(is_nds9: bool, index: usize, interrupt: InterruptRequest) -> Timer {
        Timer {
            is_nds9,
            reload: 0,
            cnt: TMCNT::new(),
            index,
            interrupt,
            // Counter Calcuation
            // Count-Up Timing
            counter: 0,
            // Regular Timing
            start_cycle: 0,
            time_till_first_clock: 0,
            timer_len: 0,
        }
    }

    pub fn clock(&mut self) -> bool {
        assert!(self.is_count_up());
        if self.cnt.start {
            let (new_counter, overflowed) = self.counter.overflowing_add(1);
            if overflowed {
                self.counter = self.reload;
                return true
            } else { self.counter = new_counter }
        }
        false
    }

    fn calc_counter(&self, global_cycle: usize) -> u16 {
        let cycles_passed = global_cycle - self.start_cycle;
        // Counter stores the reload value
        if cycles_passed >= self.time_till_first_clock {
            let cycles_passed = cycles_passed - self.time_till_first_clock;
            self.counter + 1 + cycles_passed as u16 / Timers::PRESCALERS[self.cnt.prescaler as usize] as u16
        } else { self.counter }
    }

    pub fn reload(&mut self) { self.counter = self.reload }

    pub fn create_event(&mut self, scheduler: &mut Scheduler, delay: usize) {
        let global_cycle = scheduler.cycle + delay;
        self.start_cycle = global_cycle;
        // Syncs prescaler to global cycle
        let prescaler = Timers::PRESCALERS[self.cnt.prescaler as usize];
        // Add 1 for 1 cycle delay in timer start
        self.time_till_first_clock = prescaler - (global_cycle + 1) % prescaler;
        self.timer_len = prescaler * (0x10000 - self.reload as usize - 1);
        scheduler.add(Event {
            cycle: global_cycle + self.time_till_first_clock + self.timer_len,
            event_type: EventType::TimerOverflow(self.is_nds9, self.index),
        });
    }

    pub fn is_count_up(&self) -> bool { self.cnt.count_up }

    pub fn read(&self, scheduler: &Scheduler, byte: usize) -> u8 {
        let global_cycle = scheduler.cycle;
        let counter = if self.is_count_up() || !self.cnt.start { self.counter } else { self.calc_counter(global_cycle) };
        match byte {
            0 => (counter >> 0) as u8,
            1 => (counter >> 8) as u8,
            2 | 3 => self.cnt.read(byte - 2),
            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, scheduler: &mut Scheduler, byte: usize, value: u8) {
        let global_cycle = scheduler.cycle;
        match byte {
            0 => self.reload = self.reload & !0x00FF | (value as u16) << 0,
            1 => self.reload = self.reload & !0xFF00 | (value as u16) << 8,
            2 => {
                scheduler.remove(EventType::TimerOverflow(self.is_nds9, self.index));
                let prev_start = self.cnt.start;
                if !self.is_count_up() && self.cnt.start {
                    self.counter = self.calc_counter(global_cycle);
                }
                self.cnt.write(scheduler, 0, value);
                if !self.is_count_up() {
                    if !prev_start && self.cnt.start {
                        self.reload();
                        self.create_event(scheduler, 1);
                    } else if self.cnt.start {
                        self.create_event(scheduler, 0);
                    }
                } else {
                    if !prev_start && self.cnt.start {
                        self.counter = self.reload;
                    }
                }
            },
            3 => { self.cnt.write(scheduler, 1, value); () },
            _ => unreachable!(),
        }
    }
}


#[derive(Clone, Copy)]
pub struct TMCNT {
    pub prescaler: u8,
    pub count_up: bool,
    pub irq: bool,
    pub start: bool,
}

impl IORegister for TMCNT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.start as u8) << 7 | (self.irq as u8) << 6 | (self.count_up as u8) << 2 | self.prescaler,
            1 => 0,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8){
        match byte {
            0 => {
                self.start = value >> 7 & 0x1 != 0;
                self.irq = value >> 6 & 0x1 != 0;
                self.count_up = value >> 2 & 0x1 != 0;
                self.prescaler = value & 0x3;
            },
            1 => (),
            _ => unreachable!(),
        }
    }
}

impl TMCNT {
    pub fn new() -> TMCNT {
        TMCNT {
            prescaler: 0,
            count_up: false,
            irq: false,
            start: false,
        }
    }
}
