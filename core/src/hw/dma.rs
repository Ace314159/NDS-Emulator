use super::{HW, mmu::IORegister, Scheduler, Event};

pub struct DMAController {
    pub channels: [DMAChannel; 4],
}

impl DMAController {
    pub fn new(is_nds9: bool) -> Self {
        DMAController {
            channels: [
                DMAChannel::new(is_nds9, 0),
                DMAChannel::new(is_nds9, 1),
                DMAChannel::new(is_nds9, 2),
                DMAChannel::new(is_nds9, 3),
            ],
        }
    }

    pub fn read(&self, channel: usize, addr: u32) -> u8 {
        self.channels[channel].read((addr & 0xFF) as usize)
    }

    pub fn write(&mut self, channel: usize, scheduler: &mut Scheduler, addr: u32, value: u8) {
        self.channels[channel].write(scheduler, (addr & 0xFF) as usize, value);
    }
}

pub struct DMAChannel {
    pub num: usize,
    pub is_nds9: bool,
    pub sad_latch: u32,
    pub dad_latch: u32,
    pub count_latch: u32,

    pub cnt: DMACNT,
    sad: Address,
    dad: Address,
    count: Count,
}

impl DMAChannel {
    pub fn new(is_nds9: bool, num: usize) -> Self {
        DMAChannel {
            num,
            is_nds9,
            sad_latch: 0,
            dad_latch: 0,
            count_latch: 0,

            cnt: DMACNT::new(is_nds9, num),
            sad: Address::new(if is_nds9 { 0x0FFF_FFFF } else { if num == 0 { 0x07FF_FFFF } else { 0x0FFF_FFFF} }),
            dad: Address::new(if is_nds9 { 0x0FFF_FFFF } else { if num == 3 { 0x07FF_FFFF } else { 0x0FFF_FFFF} }),
            count: Count::new(if is_nds9 { 0x1F_FFFF } else { if num == 3 { 0xFFFF } else { 0x3FFF }}),
        }
    }

    pub fn latch(&mut self) {
        self.sad_latch = self.sad.addr & self.sad.mask;
        self.dad_latch = self.dad.addr & self.sad.mask;
        let count = self.count.count & self.count.mask;
        self.count_latch = if count == 0 { self.count.mask + 1 } else { count as u32 };
    }
}

impl IORegister for DMAChannel {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0x0 => self.sad.read(0),
            0x1 => self.sad.read(1),
            0x2 => self.sad.read(2),
            0x3 => self.sad.read(3),
            0x4 => self.dad.read(0),
            0x5 => self.dad.read(1),
            0x6 => self.dad.read(2),
            0x7 => self.dad.read(3),
            0x8 => self.count.read(0),
            0x9 => self.count.read(1),
            0xA => self.cnt.read(0),
            0xB => self.cnt.read(1),
            _ => unreachable!(),
        }
    }

    fn write(&mut self, scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0x0 => self.sad.write(scheduler, 0, value),
            0x1 => self.sad.write(scheduler, 1, value),
            0x2 => self.sad.write(scheduler, 2, value),
            0x3 => self.sad.write(scheduler, 3, value),
            0x4 => self.dad.write(scheduler, 0, value),
            0x5 => self.dad.write(scheduler, 1, value),
            0x6 => self.dad.write(scheduler, 2, value),
            0x7 => self.dad.write(scheduler, 3, value),
            0x8 => self.count.write(scheduler, 0, value),
            0x9 => self.count.write(scheduler, 1, value),
            0xA => self.cnt.write(scheduler, 0, value),
            0xB => {
                let prev_enable = self.cnt.enable;
                self.cnt.write(scheduler, 1, value);
                if !prev_enable && self.cnt.enable {
                    self.latch();
                    info!("Scheduled {:?} ARM{} DMA{}: Writing {} values to {:08X} from {:08X}, size: {}",
                    self.cnt.start_timing, if self.is_nds9 { 9 } else { 7 }, self.num, self.count.count,
                    self.dad.addr, self.sad.addr, if self.cnt.transfer_32 { 32 } else { 16 });
                    if self.cnt.start_timing == DMAOccasion::Immediate {
                        scheduler.run_now(Event::DMA(self.is_nds9, self.num))
                    }
                }
            },
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DMAOccasion {
    Immediate,
    VBlank,
    HBlank,
    StartOfDisplay,
    MainMemoryDisplay,
    DSCartridge,
    GBACartridge,
    GeometryCommandFIFO,
    WirelessInterrupt,
}

impl DMAOccasion {
    fn get(is_nds9: bool, dma_num: usize, start_timing: u8) -> Self {
        if is_nds9 {
            match start_timing {
                0 => DMAOccasion::Immediate,
                1 => DMAOccasion::VBlank,
                2 => DMAOccasion::HBlank,
                3 => { warn!("ARM9 Start Of Display DMA not implemented!"); DMAOccasion::StartOfDisplay },
                4 => { warn!("ARM9 Main Memory Display DMA not implemented!"); DMAOccasion::MainMemoryDisplay },
                5 => DMAOccasion::DSCartridge,
                6 => { warn!("ARM9 GBA Cartridge DMA not implemented!"); DMAOccasion::GBACartridge },
                7 => { warn!("ARM9 Geometry Command FIFO DMA not implemented!"); DMAOccasion::GeometryCommandFIFO },
                _ => unreachable!(),
            }
        } else {
            match start_timing & 0x3 {
                0 => DMAOccasion::Immediate,
                1 => { warn!("ARM7 VBlank DMA not implemented!"); DMAOccasion::VBlank },
                2 => DMAOccasion::DSCartridge,
                3 if dma_num % 2 == 0 => { warn!("ARM7 WirelessInterrupt DMA not implemented!"); DMAOccasion::WirelessInterrupt },
                3 => { warn!("ARM7 GBA Cartridge DMA not implemented!"); DMAOccasion::GBACartridge },
                _ => unreachable!(),
            }
        }
    }
}


pub struct DMACNT {
    pub dest_addr_ctrl: u8,
    pub src_addr_ctrl: u8,
    pub repeat: bool,
    pub transfer_32: bool,
    pub start_timing: DMAOccasion,
    pub irq: bool,
    pub enable: bool,

    is_nds9: bool,
    num: usize,
}

impl DMACNT {
    pub fn new(is_nds9: bool, num: usize) -> Self {
        DMACNT {
            dest_addr_ctrl: 0,
            src_addr_ctrl: 0,
            repeat: false,
            transfer_32: false,
            start_timing: DMAOccasion::Immediate,
            irq: false,
            enable: false,

            is_nds9,
            num,
        }
    }
}

impl IORegister for DMACNT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.src_addr_ctrl & 0x1) << 7 | self.dest_addr_ctrl << 5,
            1 => (self.enable as u8) << 7 | (self.irq as u8) << 6 | (self.start_timing as u8) << 3 |
                (self.transfer_32 as u8) << 2 | (self.repeat as u8) << 1 | self.src_addr_ctrl >> 1,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => {
                self.src_addr_ctrl = self.src_addr_ctrl & !0x1 | value >> 7 & 0x1;
                self.dest_addr_ctrl = value >> 5 & 0x3;
            },
            1 => {
                self.enable = value >> 7 & 0x1 != 0;
                self.irq = value >> 6 & 0x1 != 0;
                self.start_timing = DMAOccasion::get(self.is_nds9, self.num, value >> 3 & 0x7);
                self.transfer_32 = value >> 2 & 0x1 != 0;
                self.repeat = value >> 1 & 0x1 != 0;
                self.src_addr_ctrl = self.src_addr_ctrl & !0x2 | value << 1 & 0x2;
            },
            _ => unreachable!(),
        }
    }
}


pub struct Address {
    pub addr: u32,
    mask: u32,
}

impl Address {
    pub fn new(mask: u32) -> Address {
        Address {
            addr: 0,
            mask,
        }
    }
}

impl IORegister for Address {
    fn read(&self, byte: usize) -> u8 { HW::read_byte_from_value(&self.addr, byte) }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        HW::write_byte_to_value(&mut self.addr, byte, value);
    }
}

pub struct Count {
    pub count: u32,
    mask: u32,
}

impl Count {
    pub fn new(mask: u32) -> Count {
        Count {
            count: 0,
            mask,
        }
    }
}

impl IORegister for Count {
    fn read(&self, byte: usize) -> u8 { HW::read_byte_from_value(&self.count, byte) }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        HW::write_byte_to_value(&mut self.count, byte, value);
    }
}
