use std::collections::VecDeque;

use super::interrupt_controller::InterruptRequest;

pub struct IPC {
    fifocnt7: FIFOCNT,
    sync7: SYNC,
    output7: VecDeque<u32>,
    prev_value7: u32,
    fifocnt9: FIFOCNT,
    sync9: SYNC,
    output9: VecDeque<u32>,
    prev_value9: u32,
}

impl IPC {
    const FIFO_LEN: usize = 16;

    pub fn new() -> Self {
        IPC {
            fifocnt7: FIFOCNT::new(),
            sync7: SYNC::new(),
            output7: VecDeque::new(),
            prev_value7: 0,
            fifocnt9: FIFOCNT::new(),
            sync9: SYNC::new(),
            output9: VecDeque::new(),
            prev_value9: 0,
        }
    }

    pub fn read_sync7(&self, byte: usize) -> u8 {
        self.sync7.read(byte)
    }
    pub fn read_sync9(&self, byte: usize) -> u8 {
        self.sync9.read(byte)
    }
    pub fn read_fifocnt7(&self, byte: usize) -> u8 {
        self.fifocnt7.read(&self.output7, &self.output9, byte)
    }
    pub fn read_fifocnt9(&self, byte: usize) -> u8 {
        self.fifocnt9.read(&self.output9, &self.output7, byte)
    }
    pub fn arm7_recv(&mut self) -> (u32, InterruptRequest) {
        IPC::recv(
            &self.fifocnt9,
            &mut self.fifocnt7,
            &mut self.output9,
            &mut self.prev_value9,
        )
    }
    pub fn arm9_recv(&mut self) -> (u32, InterruptRequest) {
        IPC::recv(
            &self.fifocnt7,
            &mut self.fifocnt9,
            &mut self.output7,
            &mut self.prev_value7,
        )
    }

    pub fn write_sync7(&mut self, byte: usize, value: u8) -> InterruptRequest {
        self.sync7.write(&mut self.sync9, byte, value)
    }
    pub fn write_sync9(&mut self, byte: usize, value: u8) -> InterruptRequest {
        self.sync9.write(&mut self.sync7, byte, value)
    }
    pub fn write_fifocnt7(&mut self, byte: usize, value: u8) -> InterruptRequest {
        let prev_fifocnt = self.fifocnt7;
        self.fifocnt7
            .write(&mut self.output7, &mut self.prev_value7, byte, value);
        IPC::check_fifo_interrupt(&self.output7, &self.output9, &prev_fifocnt, &self.fifocnt7)
    }
    pub fn write_fifocnt9(&mut self, byte: usize, value: u8) -> InterruptRequest {
        let prev_fifocnt = self.fifocnt9;
        self.fifocnt9
            .write(&mut self.output9, &mut self.prev_value9, byte, value);
        IPC::check_fifo_interrupt(&self.output9, &self.output7, &prev_fifocnt, &self.fifocnt9)
    }
    pub fn arm7_send(&mut self, value: u32) -> InterruptRequest {
        IPC::send(&mut self.fifocnt7, &self.fifocnt9, &mut self.output7, value)
    }
    pub fn arm9_send(&mut self, value: u32) -> InterruptRequest {
        IPC::send(&mut self.fifocnt9, &self.fifocnt7, &mut self.output9, value)
    }

    fn check_fifo_interrupt(
        send_fifo: &VecDeque<u32>,
        recv_fifo: &VecDeque<u32>,
        prev_cnt: &FIFOCNT,
        new_cnt: &FIFOCNT,
    ) -> InterruptRequest {
        let empty_condition =
            send_fifo.len() == 0 && !prev_cnt.send_fifo_empty_irq && new_cnt.send_fifo_empty_irq;
        let not_empty_condition = recv_fifo.len() != 0
            && !prev_cnt.recv_fifo_not_empty_irq
            && new_cnt.recv_fifo_not_empty_irq;

        (if empty_condition {
            InterruptRequest::IPC_SEND_FIFO_EMPTY
        } else {
            InterruptRequest::empty()
        }) | (if not_empty_condition {
            InterruptRequest::IPC_RECV_FIFO_NOT_EMPTY
        } else {
            InterruptRequest::empty()
        })
    }

    fn recv(
        send_cnt: &FIFOCNT,
        recv_cnt: &mut FIFOCNT,
        recv_fifo: &mut VecDeque<u32>,
        prev_value: &mut u32,
    ) -> (u32, InterruptRequest) {
        if !recv_cnt.enable {
            return (*prev_value, InterruptRequest::empty());
        }
        assert!(send_cnt.enable); // TODO: Figure out behavior
        let interrupt = if let Some(value) = recv_fifo.pop_front() {
            *prev_value = value;
            if send_cnt.enable && send_cnt.send_fifo_empty_irq && recv_fifo.is_empty() {
                InterruptRequest::IPC_SEND_FIFO_EMPTY
            } else {
                InterruptRequest::empty()
            }
        } else {
            recv_cnt.error = true;
            InterruptRequest::empty()
        };
        (*prev_value, interrupt)
    }

    fn send(
        send_cnt: &mut FIFOCNT,
        recv_cnt: &FIFOCNT,
        send_fifo: &mut VecDeque<u32>,
        value: u32,
    ) -> InterruptRequest {
        if !send_cnt.enable {
            return InterruptRequest::empty();
        }
        let interrupt =
            if recv_cnt.enable && recv_cnt.recv_fifo_not_empty_irq && send_fifo.is_empty() {
                InterruptRequest::IPC_RECV_FIFO_NOT_EMPTY
            } else {
                InterruptRequest::empty()
            };
        if send_fifo.len() == IPC::FIFO_LEN {
            send_cnt.error = true;
        } else {
            send_fifo.push_back(value);
        }
        interrupt
    }
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

    fn write(&mut self, other: &mut Self, byte: usize, value: u8) -> InterruptRequest {
        if match byte {
            0 => false,
            1 => {
                self.output = value;
                other.input = self.output;
                self.sync_irq = value >> 6 & 0x1 != 0;
                other.sync_irq && value >> 5 & 0x1 != 0
            }
            2 => false,
            3 => false,
            _ => unreachable!(),
        } {
            InterruptRequest::IPC_SYNC
        } else {
            InterruptRequest::empty()
        }
    }
}

#[derive(Clone, Copy)]
struct FIFOCNT {
    send_fifo_empty_irq: bool,
    recv_fifo_not_empty_irq: bool,
    error: bool,
    enable: bool,
}

impl FIFOCNT {
    fn new() -> Self {
        FIFOCNT {
            send_fifo_empty_irq: false,
            recv_fifo_not_empty_irq: false,
            error: false,
            enable: false,
        }
    }

    fn read(&self, send_fifo: &VecDeque<u32>, recv_fifo: &VecDeque<u32>, byte: usize) -> u8 {
        match byte {
            0 => (self.send_fifo_empty_irq as u8) << 2 | FIFOCNT::get_fifo_status(send_fifo),
            1 => {
                (self.enable as u8) << 7
                    | (self.error as u8) << 6
                    | (self.recv_fifo_not_empty_irq as u8) << 2
                    | FIFOCNT::get_fifo_status(recv_fifo)
            }
            2 => 0,
            3 => 0,
            _ => unreachable!(),
        }
    }

    fn write(
        &mut self,
        send_fifo: &mut VecDeque<u32>,
        prev_output: &mut u32,
        byte: usize,
        value: u8,
    ) {
        match byte {
            0 => {
                self.send_fifo_empty_irq = value >> 2 & 0x1 != 0;
                if value >> 3 & 0x1 != 0 {
                    send_fifo.clear();
                    *prev_output = 0;
                }
            }
            1 => {
                self.recv_fifo_not_empty_irq = value >> 2 & 0x1 != 0;
                self.error = self.error && (value >> 6) & 0x1 == 0; // 1 means acknowledge error
                self.enable = value >> 7 & 0x1 != 0;
            }
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
