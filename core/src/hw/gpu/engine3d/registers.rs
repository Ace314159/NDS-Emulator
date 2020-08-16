use super::Engine3D;

pub struct GXSTAT {
    pub test_busy: bool, // Box, Pos, Vector Test
    pub box_test_inside: bool,
    pub mat_stack_busy: bool,
    pub mat_stack_error: bool, // Overflow or Underflow
    pub geometry_engine_busy: bool,
    pub command_fifo_irq: CommandFifoIRQ,
}

#[derive(Clone, Copy)]
pub enum CommandFifoIRQ {
    Never = 0,
    LessHalf = 1,
    Empty = 2,
}

impl From<u8> for CommandFifoIRQ {
    fn from(value: u8) -> Self {
        match value {
            0 => CommandFifoIRQ::Never,
            1 => CommandFifoIRQ::LessHalf,
            2 => CommandFifoIRQ::Empty,
            3 => panic!("Reserved Command FIFO IRQ"),
            _ => unreachable!(),
        }
    }
}

impl GXSTAT {
    pub fn new() -> Self {
        GXSTAT {
            test_busy: false, // Box, Pos, Vector Test
            box_test_inside: false,
            mat_stack_busy: false,
            mat_stack_error: false, // Overflow or Underflow
            geometry_engine_busy: false,
            command_fifo_irq: CommandFifoIRQ::from(0),
        }
    }
}


impl Engine3D {
    pub(super) fn read_gxstat(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.gxstat.box_test_inside as u8) << 1| (self.gxstat.test_busy as u8),
            1 => (self.gxstat.mat_stack_error as u8) << 7 | (self.gxstat.mat_stack_busy as u8) << 6 |
                self.proj_stack_sp << 5 | self.pos_vec_stack_sp & 0x1F,
            2 => self.gxfifo.len() as u8,
            3 => (self.gxstat.command_fifo_irq as u8) << 6 | (self.gxstat.geometry_engine_busy as u8) << 3 |
                ((self.gxfifo.len() == 0) as u8) << 2 | ((self.gxfifo.len() < Engine3D::FIFO_LEN / 2) as u8) << 1 |
                (self.gxfifo.len() >> 8) as u8,
            _ => unreachable!(),
        }
    }

    pub(super) fn write_gxstat(&mut self, _scheduler: &mut crate::hw::scheduler::Scheduler, byte: usize, value: u8) {
        match byte {
            0 | 2 => (), // Read Only
            1 => self.gxstat.mat_stack_error = self.gxstat.mat_stack_error && value & 0x80 == 0,
            3 => self.gxstat.command_fifo_irq = CommandFifoIRQ::from(value >> 6 & 0x3),
            _ => unreachable!(),
        }
    }
}
