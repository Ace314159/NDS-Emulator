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
                self.gxstat.geometry_engine_busy = true;
                error!("Scheduling Geometry Command: {:?}", command);
                scheduler.schedule(Event::GeometryCommand(command), command.command.exec_time());
            }
        }
    }

    pub fn exec_command(&mut self, command_entry: GeometryCommandEntry) {
        use GeometryCommand::*;
        let param = command_entry.param;
        self.gxstat.geometry_engine_busy = false;
        error!("Executing Geometry Command {:?}", command_entry);
        match command_entry.command {
            MtxMode => self.mtx_mode = MatrixMode::from(param as u8 & 0x3),
            MtxPop => {
                let offset = param & 0x3F;
                let offset = if offset & 0x20 != 0 { 0xC0 | offset } else { offset } as i8;
                match self.mtx_mode {
                    MatrixMode::Proj => {
                        self.proj_stack_sp -= 1;
                        assert!(self.proj_stack_sp < 1);
                        self.cur_proj = self.proj_stack[self.proj_stack_sp as usize];
                    },
                    MatrixMode::Pos | MatrixMode::PosVec => {
                        self.pos_vec_stack_sp = (self.pos_vec_stack_sp as i8 - offset) as u8;
                        assert!(self.pos_vec_stack_sp < 31);
                        self.cur_pos = self.pos_stack[self.pos_vec_stack_sp as usize];
                        self.cur_vec = self.vec_stack[self.pos_vec_stack_sp as usize];
                    },
                    MatrixMode::Texture => {
                        self.tex_stack_sp = (self.tex_stack_sp as i8 - offset) as u8;
                        assert!(self.tex_stack_sp < 31);
                        self.cur_tex = self.tex_stack[self.tex_stack_sp as usize];
                    },
                }
            },
            MtxIdentity => self.set_cur_mat(Matrix::identity()),
            PolygonAttr => self.polygon_attrs.write(param),
            TexImageParam => self.tex_params.write(param),
            SwapBuffers => {
                self.rendering = true;
                self.gxstat.geometry_engine_busy = true; // Keep busy until VBlank
            },
            Viewport => self.viewport.write(param),
        }
    }

    pub fn write_geometry_command(&mut self, scheduler: &mut Scheduler, addr: u32, value: u32) {
        use GeometryCommand::*;
        match addr & 0xFFF {
            0x440 => self.push_geometry_command(scheduler, MtxMode, value),
            0x448 => self.push_geometry_command(scheduler, MtxPop, value),
            0x454 => self.push_geometry_command(scheduler, MtxIdentity, value),
            0x4A4 => self.push_geometry_command(scheduler, PolygonAttr, value),
            0x4A8 => self.push_geometry_command(scheduler, TexImageParam, value),
            0x540 => self.push_geometry_command(scheduler, SwapBuffers, value),
            0x580 => self.push_geometry_command(scheduler, Viewport, value),
            _ => warn!("Unimplemented Geometry Command Address 0x{:X}: 0x{:X}", addr, value)
        }
    }

    fn set_cur_mat(&mut self, mat: Matrix) {
        match self.mtx_mode {
            MatrixMode::Proj => self.cur_proj = mat,
            MatrixMode::Pos => self.cur_pos = mat,
            MatrixMode::PosVec => {
                self.cur_pos = mat;
                self.cur_vec = mat;
            },
            MatrixMode::Texture => self.cur_tex = mat,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GeometryCommand {
    MtxMode = 0x10,
    MtxPop = 0x12,
    MtxIdentity = 0x15,
    PolygonAttr = 0x29,
    TexImageParam = 0x2A,
    SwapBuffers = 0x50,
    Viewport = 0x60,
}

impl GeometryCommand {
    fn exec_time(&self) -> usize {
        use GeometryCommand::*;
        match *self {
            MtxMode => 1,
            MtxPop => 36,
            MtxIdentity => 19,
            PolygonAttr => 1,
            TexImageParam => 1,
            SwapBuffers => 0,
            Viewport => 1,
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

#[derive(Clone, Copy)]
pub struct Matrix {
    elems: [FixedPoint; 16],
}

impl Matrix {
    pub fn new(elems: [(u32, u32); 16]) -> Self {
        Matrix {
            elems: [
                FixedPoint::new(elems[0].0, elems[0].1), FixedPoint::new(elems[1].0, elems[1].1),
                FixedPoint::new(elems[2].0, elems[2].1), FixedPoint::new(elems[3].0, elems[3].1),
                FixedPoint::new(elems[4].0, elems[4].1), FixedPoint::new(elems[5].0, elems[5].1),
                FixedPoint::new(elems[6].0, elems[6].1), FixedPoint::new(elems[7].0, elems[7].1),
                FixedPoint::new(elems[8].0, elems[8].1), FixedPoint::new(elems[9].0, elems[9].1),
                FixedPoint::new(elems[10].0, elems[10].1), FixedPoint::new(elems[11].0, elems[11].1),
                FixedPoint::new(elems[12].0, elems[12].1), FixedPoint::new(elems[13].0, elems[13].1),
                FixedPoint::new(elems[14].0, elems[14].1), FixedPoint::new(elems[15].0, elems[15].1),
            ],
        }
    }

    pub fn empty() -> Self {
        Matrix::new([(0, 0); 16])
    }

    pub fn identity() -> Self {
        Matrix::new([
            (1, 0), (0, 0), (0, 0), (0, 0),
            (0, 0), (1, 0), (0, 0), (0, 0),
            (0, 0), (0, 0), (1, 0), (0, 0),
            (0, 0), (0, 0), (0, 0), (1, 0),
        ])
    }
}

#[derive(Clone, Copy)]
struct FixedPoint {
    val: u32,
}

impl FixedPoint {
    pub fn new(int: u32, frac: u32) -> Self {
        FixedPoint {
            val: int << 12 | frac,
        }
    }
}
