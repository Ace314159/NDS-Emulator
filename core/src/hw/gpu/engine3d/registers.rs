use super::IORegister;

pub struct GXSTAT {
    test_busy: bool, // Box, Pos, Vector Test
    box_test_inside: bool,
    pos_vector_mat_stack_lvl: u8,
    proj_mat_stack_lvl: bool,
    mat_stack_busy: bool,
    mat_stack_error: bool, // Overflow or Underflow
    num_command_fifo_entries: u16, // 40-bit Entries
    command_fifo_less_half_full: bool,
    command_fifo_empty: bool,
    geometry_engine_busy: bool,
    command_fifo_irq: CommandFifoIRQ,
}

#[derive(Clone, Copy)]
enum CommandFifoIRQ {
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
            pos_vector_mat_stack_lvl: 0,
            proj_mat_stack_lvl: false,
            mat_stack_busy: false,
            mat_stack_error: false, // Overflow or Underflow
            num_command_fifo_entries: 0, // 40-bit Entries
            command_fifo_less_half_full: false,
            command_fifo_empty: false,
            geometry_engine_busy: false,
            command_fifo_irq: CommandFifoIRQ::from(0),
        }
    }
}

impl IORegister for GXSTAT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.box_test_inside as u8) << 1| (self.test_busy as u8),
            1 => (self.mat_stack_error as u8) << 7 | (self.mat_stack_busy as u8) << 6 |
                (self.proj_mat_stack_lvl as u8) << 5 | self.pos_vector_mat_stack_lvl,
            2 => self.num_command_fifo_entries as u8,
            3 => (self.command_fifo_irq as u8) << 6 | (self.geometry_engine_busy as u8) << 3 |
                (self.command_fifo_empty as u8) << 2 | (self.command_fifo_less_half_full as u8) << 1 |
                (self.num_command_fifo_entries >> 8) as u8,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut crate::hw::scheduler::Scheduler, byte: usize, value: u8) {
        match byte {
            0 | 2 => (), // Read Only
            1 => self.mat_stack_error = self.mat_stack_error && value & 0x80 == 0,
            3 => self.command_fifo_irq = CommandFifoIRQ::from(value >> 6 & 0x3),
            _ => unreachable!(),
        }
    }
}
