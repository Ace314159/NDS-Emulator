use super::{HW, mem::IORegister, scheduler::Scheduler};
use num_integer::Roots;

pub struct Div {
    pub cnt: DIVCNT,
    numer: MathParam,
    denom: MathParam,
    quot: MathParam,
    rem: MathParam,
}

impl Div {
    pub fn new() -> Self {
        Div {
            cnt: DIVCNT::new(),
            numer: MathParam::new(),
            denom: MathParam::new(),
            quot: MathParam::new(),
            rem: MathParam::new(),
        }
    }

    fn calc(&mut self) {
        // TODO: Take correct num of cycles
        self.cnt.div_by_0 = self.denom.value == 0;
        let (numer, denom) = match self.cnt.mode {
            0 => (self.numer.value as u32 as i32 as i64, self.denom.value as u32 as i32 as i64),
            1 => (self.numer.value as i64, self.denom.value as u32 as i32 as i64),
            2 => (self.numer.value as i64, self.denom.value as i64),
            _ => unreachable!(),
        };
        let special_invert = |num: &mut u64| *num = *num ^ 0xFFFF_FFFF_0000_0000;
        if numer == i64::MIN && denom == -1 {
            self.quot.value = numer as u64;
            self.rem.value = 0;
            if self.cnt.mode == 0 { special_invert(&mut self.quot.value) }
        } else if denom == 0 {
            if numer == 0 {
                self.quot.value = -1i64 as u64;
            } else {
                self.quot.value = (numer.signum() * -1) as u64;
            }
            self.rem.value = numer as u64;
            if self.cnt.mode == 0 { special_invert(&mut self.quot.value) }
        } else {
            self.quot.value = (numer / denom) as u64;
            self.rem.value = (numer % denom) as u64;
        }
    }
    
    pub fn read_numer(&self, byte: usize) -> u8 { self.numer.read(byte) }
    pub fn read_denom(&self, byte: usize) -> u8 { self.denom.read(byte) }
    pub fn read_quot(&self, byte: usize) -> u8 { self.quot.read(byte) }
    pub fn read_rem(&self, byte: usize) -> u8 { self.rem.read(byte) }

    pub fn write_numer(&mut self, scheduler: &mut Scheduler, byte: usize, value: u8) {
        self.numer.write(scheduler, byte, value);
        self.calc();
    }
    pub fn write_denom(&mut self, scheduler: &mut Scheduler, byte: usize, value: u8) {
        self.denom.write(scheduler, byte, value);
        self.calc();
    }
}

pub struct Sqrt {
    pub cnt: SQRTCNT,
    param: MathParam,
    result: u32,
}

impl Sqrt {
    pub fn new() -> Self {
        Sqrt {
            cnt: SQRTCNT::new(),
            param: MathParam::new(),
            result: 0,
        }
    }

    pub fn read_param(&self, byte: usize) -> u8 { self.param.read(byte) }
    pub fn read_result(&self, byte: usize) -> u8 { HW::read_byte_from_value(&self.result, byte) }

    pub fn write_param(&mut self, scheduler: &mut Scheduler, byte: usize, value: u8) {
        self.param.write(scheduler, byte, value);
        // TODO: Take correct num of cycles
        self.result = if self.cnt.is_64bit {
            self.param.value.sqrt() as u32
        } else {
            (self.param.value as u32).sqrt()
        };
    }
}

pub struct MathParam {
    value: u64,
}

impl MathParam {
    pub fn new() -> Self {
        MathParam {
            value: 0,
        }
    }
}

impl IORegister for MathParam {
    fn read(&self, byte: usize) -> u8 {
        assert!(byte < 8);
        HW::read_byte_from_value(&self.value, byte)
    }

    fn write(&mut self, _scheduler: &mut super::scheduler::Scheduler, byte: usize, value: u8) {
        assert!(byte < 8);
        HW::write_byte_to_value(&mut self.value, byte, value);
    }
}

pub struct DIVCNT {
    mode: u8,
    div_by_0: bool,
    busy: bool,
}

impl DIVCNT {
    pub fn new() -> Self {
        DIVCNT {
            mode: 0,
            div_by_0: false,
            busy: false,
        }
    }
}

impl IORegister for DIVCNT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => self.mode,
            1 => (self.busy as u8) << 7 | (self.div_by_0 as u8) << 6,
            2 ..= 3 => 0,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut super::scheduler::Scheduler, byte: usize, value: u8) {
        match byte {
            0 => self.mode = value & 0x3,
            1 ..= 3 => (),
            _ => unreachable!(),
        }
    }
}

pub struct SQRTCNT {
    is_64bit: bool,
    busy: bool,
}

impl SQRTCNT {
    pub fn new() -> Self {
        SQRTCNT {
            is_64bit: false,
            busy: false,
        }
    }
}

impl IORegister for SQRTCNT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => self.is_64bit as u8,
            1 => (self.busy as u8) << 7,
            2 ..= 3 => 0,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => self.is_64bit = value & 0x1 != 0,
            1 ..= 3 => (),
            _ => unreachable!(),
        }
    }
}
