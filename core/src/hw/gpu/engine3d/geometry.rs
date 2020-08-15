use super::{Engine3D, Event, Scheduler};


impl Engine3D {
    fn push_geometry_command(&mut self, scheduler: &mut Scheduler, command: GeometryCommand, param: u32) {
        let command = GeometryCommandEntry::new(command, param);
        if self.gxfifo.len() == 0 && self.gxpipe.len() < Engine3D::PIPE_LEN { self.gxpipe.push_back(command) }
        else if self.gxfifo.len() < Engine3D::FIFO_LEN { self.gxfifo.push_back(command) }
        else { todo!() } // TODO: Stall Bus

        self.schedule_command(scheduler);
    }

    pub fn schedule_command(&mut self, scheduler: &mut Scheduler) {
        if !self.gxstat.geometry_engine_busy { 
            if let Some(command) = self.gxpipe.pop_front() {
                    scheduler.schedule(Event::GeometryCommand(command), command.command.exec_time());
            }
        }
    }

    pub fn exec_command(&mut self, command_entry: GeometryCommandEntry) {
        use GeometryCommand::*;
        let param = command_entry.param;
        match command_entry.command {
            MtxMode => self.mtx_mode = MatrixMode::from(param as u8 & 0x3),
        }
    }

    pub fn write_geometry_command(&mut self, scheduler: &mut Scheduler, addr: u32, value: u32) {
        use GeometryCommand::*;
        match addr & 0xFFF {
            0x440 => self.push_geometry_command(scheduler, MtxMode, value),
            
            _ => warn!("Unimplemented Geometry Command Address: 0x{:X}", addr)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GeometryCommand {
    MtxMode = 0x10,
}

impl GeometryCommand {
    fn exec_time(&self) -> usize {
        use GeometryCommand::*;
        match *self {
            MtxMode => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GeometryCommandEntry {
    command: GeometryCommand,
    param: u32,
}

impl GeometryCommandEntry {
    pub fn new(command: GeometryCommand, param: u32) -> Self {
        GeometryCommandEntry {
            command,
            param,
        }
    }
}

pub enum MatrixMode {
    Proj = 0,
    Pos = 1,
    PosVec = 2,
    Texture = 3,
}

impl From<u8> for MatrixMode {
    fn from(value: u8) -> Self {
        match value {
            0 => MatrixMode::Proj,
            1 => MatrixMode::Pos,
            2 => MatrixMode::PosVec,
            3 => MatrixMode::Texture,
            _ => unreachable!(),
        }
    }
}
