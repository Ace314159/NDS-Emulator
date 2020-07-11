use bitflags::*;
use super::{mmu::IORegister, Scheduler};

#[derive(Clone, Copy)]
pub enum Key {
    A = 0,
    B = 1,
    X = 10,
    Y = 11,
    Select = 2,
    Start = 3,
    Right = 4,
    Left = 5,
    Up = 6,
    Down = 7,
    R = 8,
    L = 9,
}

pub struct Keypad {
    pub keyinput: KEYINPUT,
    pub keycnt: KEYCNT,
    pub extkeyin: EXTKEYIN,
}

impl Keypad {
    pub fn new() -> Self {
        Keypad {
            keyinput: KEYINPUT::all(),
            keycnt: KEYCNT::empty(),
            extkeyin: EXTKEYIN::new(),
        }
    }

    pub fn press_key(&mut self, key: Key) {
        if (key as usize) < 10 {
            self.keyinput.bits &= !(1 << (key as usize));
        } else {
            self.extkeyin.bits &= !(1 << (key as usize - 10));
        }
    }

    pub fn release_key(&mut self, key: Key) {
        if (key as usize) < 10 {
            self.keyinput.bits |= 1 << (key as usize);
        } else {
            self.extkeyin.bits |= 1 << (key as usize - 10);
        }
    }

    pub fn interrupt_requested(&self) -> bool {
        if self.keycnt.contains(KEYCNT::IRQ_ENABLE) {
            let irq_keys = self.keycnt - KEYCNT::IRQ_ENABLE - KEYCNT::IRQ_COND_AND;
            if self.keycnt.contains(KEYCNT::IRQ_COND_AND) { irq_keys.bits() & !self.keyinput.bits() == irq_keys.bits() }
            else { irq_keys.bits() & !self.keyinput.bits() != 0 }
        } else { false }
    }
}


bitflags! {
    pub struct KEYINPUT: u16 {
        const A = 1 << 0;
        const B = 1 << 1;
        const SELECT = 1 << 2;
        const START = 1 << 3;
        const RIGHT = 1 << 4;
        const LEFT = 1 << 5;
        const UP = 1 << 6;
        const DOWN = 1 << 7;
        const R = 1 << 8;
        const L = 1 << 9;
    }
}

bitflags! {
    pub struct KEYCNT: u16 {
        const A = 1 << 0;
        const B = 1 << 1;
        const SELECT = 1 << 2;
        const START = 1 << 3;
        const RIGHT = 1 << 4;
        const LEFT = 1 << 5;
        const UP = 1 << 6;
        const DOWN = 1 << 7;
        const R = 1 << 8;
        const L = 1 << 9;
        const IRQ_ENABLE = 1 << 14;
        const IRQ_COND_AND = 1 << 15;
    }
}

bitflags! {
    pub struct EXTKEYIN: u8 {
        const X = 1 << 0;
        const Y = 1 << 1;
        const DEBUG = 1 << 3;
        const PEN_DOWN = 1 << 6;
        const HINGE_CLOSED = 1 << 7;
    }
}

impl IORegister for KEYINPUT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => self.bits as u8,
            1 => (self.bits >> 8) as u8,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, _byte: usize, _value: u8) {}
}

impl IORegister for KEYCNT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => self.bits as u8,
            1 => (self.bits >> 8) as u8,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => self.bits = self.bits & !0x00FF | (value as u16) & KEYCNT::all().bits,
            1 => self.bits = self.bits & !0xFF00 | (value as u16) << 8 & KEYCNT::all().bits,
            _ => unreachable!(),
        }
    }
}

impl IORegister for EXTKEYIN {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => self.bits | EXTKEYIN::MASK,
            1 => 0,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, _byte: usize, _value: u8) {}
}

impl EXTKEYIN {
    const MASK: u8 = 0b0011_0100;

    pub fn new() -> Self {
        EXTKEYIN::PEN_DOWN | EXTKEYIN::DEBUG | EXTKEYIN::Y | EXTKEYIN::X
    }
}

