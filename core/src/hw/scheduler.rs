use std::cmp::Reverse;

use priority_queue::PriorityQueue;

use super::{
    AccessType,
    dma::{DMAChannel, DMAOccasion},
    mmu::MemoryValue,
    InterruptRequest,
    HW
};

impl HW {
    pub fn handle_events(&mut self, arm7_cycles: usize) {
        self.scheduler.cycle += arm7_cycles;
        while let Some(event) = self.scheduler.get_next_event() {
            self.handle_event(event);
        }
    }

    pub fn handle_event(&mut self, event: EventType) {
        match event {
            EventType::DMA(is_nds9, num) => {
                let channel = self.get_channel(is_nds9, num);
                // TODO: Clean Up
                if channel.cnt.transfer_32 {
                    if is_nds9 {
                        self.run_dma(true, num, &HW::arm9_get_access_time::<u32>,
                            &HW::arm9_read::<u32>, &HW::arm9_write::<u32>);
                    } else {
                        self.run_dma(false, num, &HW::arm7_get_access_time::<u32>,
                            &HW::arm7_read::<u32>, &HW::arm7_write::<u32>);
                    }
                } else {
                    if is_nds9 {
                        self.run_dma(true, num, &HW::arm9_get_access_time::<u16>,
                            &HW::arm9_read::<u16>, &HW::arm9_write::<u16>);
                    } else {
                        self.run_dma(false, num, &HW::arm7_get_access_time::<u16>,
                            &HW::arm7_read::<u16>, &HW::arm7_write::<u16>);
                    }
                }
            },
            EventType::VBlank => {
                let mut events = Vec::new();
                for channel in self.dma9.channels.iter().chain(self.dma7.channels.iter()) {
                    if channel.cnt.start_timing == DMAOccasion::VBlank {
                        events.push(EventType::DMA(channel.is_nds9, channel.num));
                    }
                }
                for event in events.iter() { self.handle_event(*event) }
            },
            EventType::HBlank => {
                let mut events = Vec::new();
                for channel in self.dma9.channels.iter().chain(self.dma7.channels.iter()) {
                    if channel.cnt.start_timing == DMAOccasion::HBlank {
                        events.push(EventType::DMA(channel.is_nds9, channel.num));
                    }
                }
                for event in events.iter() { self.handle_event(*event) }
            },
            EventType::TimerOverflow(is_nds9, timer) => {
                let (timers, interrupts) = if is_nds9 {
                    (&mut self.timers9, &mut self.interrupts9)
                } else {
                    (&mut self.timers7, &mut self.interrupts7)
                };
                if timers.timers[timer].cnt.irq {
                    interrupts.request |= timers.timers[timer].interrupt
                }
                // Cascade Timers
                if timer + 1 < timers.timers.len() && timers.timers[timer + 1].is_count_up() {
                    if timers.timers[timer + 1].clock() { self.handle_event(EventType::TimerOverflow(is_nds9, timer + 1)) }
                }
                // TODO: Can I move this up to avoid recreating timers
                let timers = if is_nds9 { &mut self.timers9 } else { &mut self.timers7 };
                if !timers.timers[timer].is_count_up() {
                    timers.timers[timer].reload();
                    timers.timers[timer].create_event(&mut self.scheduler, 0);
                }
            },
        }
    }

    fn get_channel(&mut self, is_nds9: bool, num: usize) -> &mut DMAChannel {
        if is_nds9 { &mut self.dma9.channels[num] } else { &mut self.dma7.channels[num] }
    } 

    fn run_dma<A, R, W, T: MemoryValue>(&mut self, is_nds9: bool, num: usize, access_time_fn: A, read_fn: R, write_fn: W)
        where A: Fn(&mut HW, AccessType, u32) -> usize, R: Fn(&mut HW, u32) -> T, W: Fn(&mut HW, u32, T) {
        let channel = self.get_channel(is_nds9, num);
        let count = channel.count_latch;
        let mut src_addr = channel.sad_latch;
        let mut dest_addr = channel.dad_latch;
        let src_addr_ctrl = channel.cnt.src_addr_ctrl;
        let dest_addr_ctrl = channel.cnt.dest_addr_ctrl;
        let transfer_32 = channel.cnt.transfer_32;
        let irq = channel.cnt.irq;
        channel.cnt.enable = channel.cnt.start_timing != DMAOccasion::Immediate && channel.cnt.repeat;
        info!("Running DMA{}: Writing {} values to {:08X} from {:08X}, size: {}", num, count, dest_addr,
        src_addr, if transfer_32 { 32 } else { 16 });

        let (addr_change, addr_mask) = if transfer_32 { (4, 0x3) } else { (2, 0x1) };
        src_addr &= !addr_mask;
        dest_addr &= !addr_mask;
        let mut first = true;
        let original_dest_addr = dest_addr;
        let mut cycles_passed = 0;
        for _ in 0..count {
            let cycle_type = if first { AccessType::N } else { AccessType::S };
            cycles_passed += access_time_fn(self, cycle_type, src_addr);
            cycles_passed += access_time_fn(self, cycle_type, dest_addr);
            let value = read_fn(self, src_addr);
            write_fn(self, dest_addr, value);

            src_addr = match src_addr_ctrl {
                0 => src_addr.wrapping_add(addr_change),
                1 => src_addr.wrapping_sub(addr_change),
                2 => src_addr,
                _ => panic!("Invalid DMA Source Address Control!"),
            };
            dest_addr = match dest_addr_ctrl {
                0 | 3 => dest_addr.wrapping_add(addr_change),
                1 => dest_addr.wrapping_sub(addr_change),
                2 => dest_addr,
                _ => unreachable!(),
            };
            first = false;
        }
        let channel = self.get_channel(is_nds9, num);
        channel.sad_latch = src_addr;
        channel.dad_latch = dest_addr;
        // if channel.cnt.enable { channel.count_latch = channel.count.count as u32 } // Only reload Count - TODO: Why?
        if dest_addr_ctrl == 3 { channel.dad_latch = original_dest_addr }
        cycles_passed += 2; // 2 I cycles

        // TODO: Don't halt CPU if PC is in TCM
        self.clock(cycles_passed);
        
        if irq {
            let interrupt = match num {
                0 => InterruptRequest::DMA0,
                1 => InterruptRequest::DMA1,
                2 => InterruptRequest::DMA2,
                3 => InterruptRequest::DMA3,
                _ => unreachable!(),
            };
            self.interrupts7.request |= interrupt;
            self.interrupts9.request |= interrupt;
        }
    }
}

pub struct Scheduler {
    pub cycle: usize,
    event_queue: PriorityQueue<EventType, Reverse<usize>>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        let queue = PriorityQueue::new();
        Scheduler {
            cycle: 0,
            event_queue: queue,
        }
    }

    pub fn get_next_event(&mut self) -> Option<EventType> {
        if self.event_queue.len() == 0 { return None }
        let (_event_type, cycle) = self.event_queue.peek().unwrap();
        if Reverse(self.cycle) <= *cycle {
            Some(self.event_queue.pop().unwrap().0)
        } else { None }
    }

    pub fn add(&mut self, event: Event) {
        self.event_queue.push(event.event_type, Reverse(event.cycle));
    }

    pub fn run_now(&mut self, event_type: EventType) {
        self.event_queue.push(event_type, Reverse(self.cycle + 1));
    }

    pub fn remove(&mut self, event_type: EventType) {
        self.event_queue.remove(&event_type);
    }
}

pub struct Event {
    pub cycle: usize,
    pub event_type: EventType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventType {
    DMA(bool, usize),
    VBlank,
    HBlank,
    TimerOverflow(bool, usize),
}
