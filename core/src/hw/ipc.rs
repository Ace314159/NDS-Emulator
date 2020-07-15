use std::collections::VecDeque;

pub struct IPC {
    fifocnt7: FIFOCNT,
    sync7: SYNC,
    output7: VecDeque<u32>,
    fifocnt9: FIFOCNT,
    sync9: SYNC,
    output9: VecDeque<u32>,
}

impl IPC {
    const FIFO_LEN: usize = 16;
    
    pub fn new() -> Self {
        IPC {
            fifocnt7: FIFOCNT::new(),
            sync7: SYNC::new(),
            output7: VecDeque::new(),
            fifocnt9: FIFOCNT::new(),
            sync9: SYNC::new(),
            output9: VecDeque::new(),
        }
    }

    pub fn read_sync7(&self, byte: usize) -> u8 { self.sync7.read(byte) }
    pub fn read_sync9(&self, byte: usize) -> u8 { self.sync9.read(byte) }
    pub fn write_sync7(&mut self, byte: usize, value: u8) -> bool { self.sync7.write(&mut self.sync9, byte, value) }
    pub fn write_sync9(&mut self, byte: usize, value: u8) -> bool { self.sync9.write(&mut self.sync7, byte, value) }
    
}

struct SYNC {
    input: u8,
    output: u8,
    sync_irq: bool,
}

impl SYNC {
    fn new() -> Self {
        SYNC {
            input: 0,
            output: 0,
            sync_irq: false,
        }
    }

    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => self.input,
            1 => (self.sync_irq as u8) << 6 | self.output,
            2 => 0,
            3 => 0,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, other: &mut Self, byte: usize, value: u8) -> bool {
        match byte {
            0 => false,
            1 => {
                self.output = value;
                other.input = self.output;
                self.sync_irq = value >> 6 & 0x1 != 0;
                other.sync_irq && value >> 5 & 0x1 != 0
            },
            2 => false,
            3 => false,
            _ => unreachable!(),
        }
    }
}

struct FIFOCNT {
    send_fifo_empty_irq: bool,
    recv_fifo_empty_irq: bool,
    error: bool,
    enable: bool,
}

impl FIFOCNT {
    fn new() -> Self {
        FIFOCNT {
            send_fifo_empty_irq: false,
            recv_fifo_empty_irq: false,
            error: false,
            enable: false,
        }
    }

    fn read(&self, send_fifo: &VecDeque<u32>, recv_fifo: &VecDeque<u32>, byte: usize) -> u8 {
        match byte {
            0 => (self.send_fifo_empty_irq as u8) << 2 | FIFOCNT::get_fifo_status(send_fifo),
            1 => (self.enable as u8) << 7 | (self.error as u8) << 6 |
                (self.recv_fifo_empty_irq as u8) << 2 | FIFOCNT::get_fifo_status(recv_fifo),
            2 => 0,
            3 => 0,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, send_fifo: &mut VecDeque<u32>, recv_fifo: &mut VecDeque<u32>, byte: usize, value: u8) {
        match byte {
            0 => {
                self.send_fifo_empty_irq = value >> 2 & 0x1 != 0;
                if value >> 3 & 0x1 != 0 {
                    send_fifo.clear();
                }
            },
            1 => {
                self.recv_fifo_empty_irq = value >> 2 & 0x1 != 0;
                self.error = self.error && (value >> 6) & 0x1 != 0; // 1 means acknowledge error
                self.enable = value >> 7 & 0x1 != 0;
            },
            2 => (),
            3 => (),
            _ => unreachable!(),
        }
    }

    fn get_fifo_status(fifo: &VecDeque<u32>) -> u8 {
        assert!(fifo.len() <= IPC::FIFO_LEN);
        ((fifo.len() == IPC::FIFO_LEN) as u8) << 1 | fifo.is_empty() as u8
    }
}
