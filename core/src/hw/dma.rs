use super::{
    interrupt_controller::InterruptRequest,
    mem::{AccessType, IORegister, MemoryValue},
    scheduler::{Event, Scheduler},
    HW,
};

pub struct DMAController {
    channels: [DMAChannel; 4],
    pub by_type: [Vec<usize>; DMAOccasion::num()],
}

impl DMAController {
    pub fn new(is_nds9: bool) -> Self {
        let mut controller = DMAController {
            channels: [
                DMAChannel::new(is_nds9, 0),
                DMAChannel::new(is_nds9, 1),
                DMAChannel::new(is_nds9, 2),
                DMAChannel::new(is_nds9, 3),
            ],
            by_type: Default::default(), // TODO: Use ArrayVec or smth maybe?
        };
        for i in 0..4 {
            controller.by_type[controller.channels[i].cnt.start_timing as usize].push(i)
        }
        controller
    }

    pub fn read(&self, channel: usize, addr: u32) -> u8 {
        self.channels[channel].read((addr & 0xFF) as usize)
    }

    pub fn write(&mut self, channel: usize, scheduler: &mut Scheduler, addr: u32, value: u8) {
        let prev_start_timing = self.channels[channel].cnt.start_timing;
        let prev_enable = self.channels[channel].cnt.enable;
        self.channels[channel].write(scheduler, (addr & 0xFF) as usize, value);
        let new_start_timing = self.channels[channel].cnt.start_timing;
        let new_enable = self.channels[channel].cnt.enable;
        // TODO: Only call this when the upper byte of cnt is written to
        if prev_enable != new_enable || prev_start_timing != new_start_timing {
            if prev_enable {
                let vec = &mut self.by_type[prev_start_timing as usize];
                let pos = vec.iter().position(|i| *i == channel);
                vec.swap_remove(pos.unwrap());
            }
            if new_enable {
                self.by_type[new_start_timing as usize].push(channel);
            }
        }
        if !prev_enable && new_enable {
            let channel = &mut self.channels[channel];
            channel.latch();
            info!(
                "Scheduled {:?} ARM{} DMA{}: Writing {} values to {:08X} from {:08X}, size: {}",
                channel.cnt.start_timing,
                if channel.is_nds9 { 9 } else { 7 },
                channel.num,
                channel.cnt.count,
                channel.dad.addr,
                channel.sad.addr,
                if channel.cnt.transfer_32 { 32 } else { 16 }
            );
            match channel.cnt.start_timing {
                DMAOccasion::Immediate => {
                    scheduler.run_now(Event::DMA(channel.is_nds9, channel.num), HW::on_dma)
                }
                DMAOccasion::GeometryCommandFIFO => scheduler.run_now(
                    Event::CheckGeometryCommandFIFO,
                    HW::check_geometry_command_fifo_handler,
                ),
                _ => (),
            }
        }
    }

    pub fn disable(&mut self, channel: usize) {
        let vec = &mut self.by_type[self.channels[channel].cnt.start_timing as usize];
        let pos = vec.iter().position(|i| *i == channel);
        vec.swap_remove(pos.unwrap());
    }
}

impl std::ops::Index<usize> for DMAController {
    type Output = DMAChannel;

    fn index(&self, index: usize) -> &Self::Output {
        &self.channels[index]
    }
}

impl std::ops::IndexMut<usize> for DMAController {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.channels[index]
    }
}

impl HW {
    fn on_dma(&mut self, event: Event) {
        let (is_nds9, num) = match event {
            Event::DMA(is_nds9, num) => (is_nds9, num),
            _ => unreachable!(),
        };
        if self.dmas[is_nds9 as usize][num].cnt.transfer_32 {
            if is_nds9 {
                self.run_dma::<_, _, _, _, true>(
                    num,
                    &HW::arm9_get_access_time::<u32>,
                    &HW::arm9_read::<u32>,
                    &HW::arm9_write::<u32>,
                );
            } else {
                self.run_dma::<_, _, _, _, false>(
                    num,
                    &HW::arm7_get_access_time::<u32>,
                    &HW::arm7_read::<u32>,
                    &HW::arm7_write::<u32>,
                );
            }
        } else {
            if is_nds9 {
                self.run_dma::<_, _, _, _, true>(
                    num,
                    &HW::arm9_get_access_time::<u16>,
                    &HW::arm9_read::<u16>,
                    &HW::arm9_write::<u16>,
                );
            } else {
                self.run_dma::<_, _, _, _, false>(
                    num,
                    &HW::arm7_get_access_time::<u16>,
                    &HW::arm7_read::<u16>,
                    &HW::arm7_write::<u16>,
                );
            }
        }
    }

    fn run_dma<A, R, W, T: MemoryValue, const IS_NDS9: bool>(
        &mut self,
        num: usize,
        access_time_fn: A,
        read_fn: R,
        write_fn: W,
    ) where
        A: Fn(&mut HW, AccessType, u32) -> usize,
        R: Fn(&mut HW, u32) -> T,
        W: Fn(&mut HW, u32, T),
    {
        let i = IS_NDS9 as usize;
        let channel = &mut self.dmas[i][num];
        let count = channel.count_latch;
        let mut src_addr = channel.sad_latch;
        let mut dest_addr = channel.dad_latch;
        let src_addr_ctrl = channel.cnt.src_addr_ctrl;
        let dest_addr_ctrl = channel.cnt.dest_addr_ctrl;
        let transfer_32 = channel.cnt.transfer_32;
        let irq = channel.cnt.irq;
        channel.cnt.enable =
            channel.cnt.start_timing != DMAOccasion::Immediate && channel.cnt.repeat;
        info!(
            "Running {:?} ARM{} DMA{}: Writing {} values to {:08X} from {:08X}, size: {}",
            channel.cnt.start_timing,
            if IS_NDS9 { 9 } else { 7 },
            num,
            count,
            dest_addr,
            src_addr,
            if transfer_32 { 32 } else { 16 }
        );

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
        let channel = &mut self.dmas[i][num];
        channel.sad_latch = src_addr;
        channel.dad_latch = dest_addr;
        // if channel.cnt.enable { channel.count_latch = channel.count.count as u32 } // Only reload Count - TODO: Why?
        if dest_addr_ctrl == 3 {
            channel.dad_latch = original_dest_addr
        }
        cycles_passed += 2; // 2 I cycles

        if !channel.cnt.enable {
            self.dmas[i].disable(num)
        }

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
            self.interrupts[0].request |= interrupt;
            self.interrupts[1].request |= interrupt;
        }
    }

    fn check_geometry_command_fifo_handler(&mut self, _event: Event) {
        self.check_geometry_command_fifo();
    }

    pub fn check_geometry_command_fifo(&mut self) {
        if self.gpu.engine3d.should_run_fifo() {
            self.run_dmas(DMAOccasion::GeometryCommandFIFO);
        }
    }

    pub fn run_dmas(&mut self, occasion: DMAOccasion) {
        let mut events = Vec::new();
        for dma in self.dmas.iter() {
            for num in dma.by_type[occasion as usize].iter() {
                events.push(Event::DMA(true, *num));
            }
        }
        for event in events.drain(..) {
            self.on_dma(event)
        }
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
            sad: Address::new(if is_nds9 {
                0x0FFF_FFFF
            } else {
                if num == 0 {
                    0x07FF_FFFF
                } else {
                    0x0FFF_FFFF
                }
            }),
            dad: Address::new(if is_nds9 {
                0x0FFF_FFFF
            } else {
                if num == 3 {
                    0x07FF_FFFF
                } else {
                    0x0FFF_FFFF
                }
            }),
        }
    }

    pub fn latch(&mut self) {
        self.sad_latch = self.sad.addr & self.sad.mask;
        self.dad_latch = self.dad.addr & self.sad.mask;
        let count = self.cnt.count & self.cnt.count_mask;
        self.count_latch = if count == 0 {
            self.cnt.count_mask + 1
        } else {
            count
        };
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
            0x8 => self.cnt.read(0),
            0x9 => self.cnt.read(1),
            0xA => self.cnt.read(2),
            0xB => self.cnt.read(3),
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
            0x8 => self.cnt.write(scheduler, 0, value),
            0x9 => self.cnt.write(scheduler, 1, value),
            0xA => self.cnt.write(scheduler, 2, value),
            0xB => self.cnt.write(scheduler, 3, value),
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DMAOccasion {
    Immediate = 0,
    VBlank = 1,
    HBlank = 2,
    StartOfDisplay = 3,
    MainMemoryDisplay = 4,
    DSCartridge = 5,
    GBACartridge = 6,
    GeometryCommandFIFO = 7,
    WirelessInterrupt = 8,
}

impl DMAOccasion {
    const fn num() -> usize {
        9
    }

    fn get(is_nds9: bool, dma_num: usize, start_timing: u8) -> Self {
        if is_nds9 {
            match start_timing {
                0 => DMAOccasion::Immediate,
                1 => DMAOccasion::VBlank,
                2 => DMAOccasion::HBlank,
                3 => {
                    warn!("ARM9 Start Of Display DMA not implemented!");
                    DMAOccasion::StartOfDisplay
                }
                4 => {
                    warn!("ARM9 Main Memory Display DMA not implemented!");
                    DMAOccasion::MainMemoryDisplay
                }
                5 => DMAOccasion::DSCartridge,
                6 => {
                    warn!("ARM9 GBA Cartridge DMA not implemented!");
                    DMAOccasion::GBACartridge
                }
                7 => DMAOccasion::GeometryCommandFIFO,
                _ => unreachable!(),
            }
        } else {
            match start_timing & 0x3 {
                0 => DMAOccasion::Immediate,
                1 => {
                    warn!("ARM7 VBlank DMA not implemented!");
                    DMAOccasion::VBlank
                }
                2 => DMAOccasion::DSCartridge,
                3 if dma_num % 2 == 0 => {
                    warn!("ARM7 WirelessInterrupt DMA not implemented!");
                    DMAOccasion::WirelessInterrupt
                }
                3 => {
                    warn!("ARM7 GBA Cartridge DMA not implemented!");
                    DMAOccasion::GBACartridge
                }
                _ => unreachable!(),
            }
        }
    }
}

pub struct DMACNT {
    count: u32,
    pub count_latch: u32,
    pub dest_addr_ctrl: u8,
    pub src_addr_ctrl: u8,
    pub repeat: bool,
    pub transfer_32: bool,
    pub start_timing: DMAOccasion,
    pub irq: bool,
    pub enable: bool,

    is_nds9: bool,
    num: usize,
    count_mask: u32,
}

impl DMACNT {
    pub fn new(is_nds9: bool, num: usize) -> Self {
        DMACNT {
            count: 0,
            count_latch: 0,
            dest_addr_ctrl: 0,
            src_addr_ctrl: 0,
            repeat: false,
            transfer_32: false,
            start_timing: DMAOccasion::Immediate,
            irq: false,
            enable: false,

            is_nds9,
            num,
            count_mask: if is_nds9 {
                0x1F_FFFF
            } else {
                if num == 3 {
                    0xFFFF
                } else {
                    0x3FFF
                }
            },
        }
    }
}

impl IORegister for DMACNT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 | 1 => HW::read_byte_from_value(&self.count, byte),
            2 => {
                (self.src_addr_ctrl & 0x1) << 7
                    | self.dest_addr_ctrl << 5
                    | (self.count >> 16) as u8
            }
            3 => {
                (self.enable as u8) << 7
                    | (self.irq as u8) << 6
                    | (self.start_timing as u8) << 3
                    | (self.transfer_32 as u8) << 2
                    | (self.repeat as u8) << 1
                    | self.src_addr_ctrl >> 1
            }
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 | 1 => HW::write_byte_to_value(&mut self.count, byte, value),
            2 => {
                if self.is_nds9 {
                    HW::write_byte_to_value(&mut self.count, 2, value & 0x1F)
                }
                self.src_addr_ctrl = self.src_addr_ctrl & !0x1 | value >> 7 & 0x1;
                self.dest_addr_ctrl = value >> 5 & 0x3;
            }
            3 => {
                self.enable = value >> 7 & 0x1 != 0;
                self.irq = value >> 6 & 0x1 != 0;
                self.start_timing = DMAOccasion::get(self.is_nds9, self.num, value >> 3 & 0x7);
                self.transfer_32 = value >> 2 & 0x1 != 0;
                self.repeat = value >> 1 & 0x1 != 0;
                self.src_addr_ctrl = self.src_addr_ctrl & !0x2 | value << 1 & 0x2;
            }
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
        Address { addr: 0, mask }
    }
}

impl IORegister for Address {
    fn read(&self, byte: usize) -> u8 {
        HW::read_byte_from_value(&self.addr, byte)
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        HW::write_byte_to_value(&mut self.addr, byte, value);
    }
}
