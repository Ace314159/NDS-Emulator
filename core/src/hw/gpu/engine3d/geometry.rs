pub type FixedPoint = simba::scalar::FixedI32::<fixed::types::extra::U12>;
pub type Matrix = nalgebra::Matrix4<FixedPoint>;
pub use num_traits::identities::Zero;

use super::Engine3D;


impl Engine3D {
    fn push_geometry_command(&mut self, command: GeometryCommand, param: u32) {
        let command = GeometryCommandEntry::new(command, param);
        if self.gxfifo.len() == 0 && self.gxpipe.len() < Engine3D::PIPE_LEN { self.gxpipe.push_back(command) }
        else if self.gxfifo.len() < Engine3D::FIFO_LEN { self.gxfifo.push_back(command) }
        else { todo!() } // TODO: Stall Bus
    }

    pub fn exec_command(&mut self, command_entry: GeometryCommandEntry) {
        self.params.push(command_entry.param);
        self.cycles_ahead -= 1; // 1 cycle for processing command
        if self.params.len() < command_entry.command.num_params() {
            if self.params.len() > 1 { assert_eq!(self.prev_command, command_entry.command) }
            self.prev_command = command_entry.command;
            return
        }

        use GeometryCommand::*;
        let param = command_entry.param;
        self.gxstat.geometry_engine_busy = false;
        error!("Executing Geometry Command {:?} {:?}", command_entry.command, self.params);
        match command_entry.command {
            Unimplemented => (),
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
            MtxIdentity => self.apply_cur_mat(|_| Matrix::identity()),
            MtxMult4x4 => {
                assert_eq!(self.params.len(), 16);
                let mat = Matrix::from_fn(
                    |i, j| create_fixed_point(self.params[i * 4 + j])
                );
                self.apply_cur_mat(|old| mat * old);
            },
            PolygonAttr => self.polygon_attrs.write(param),
            TexImageParam => self.tex_params.write(param),
            SwapBuffers => {
                self.rendering = true;
                self.gxstat.geometry_engine_busy = true; // Keep busy until VBlank
            },
            Viewport => self.viewport.write(param),
        }
        self.params.clear();
        self.cycles_ahead -= command_entry.command.exec_time() as i32;
    }

    pub fn write_geometry_command(&mut self, addr: u32, value: u32) {
        let command = GeometryCommand::from_addr(addr & 0xFFF);
        if command != GeometryCommand::Unimplemented {
            self.push_geometry_command(command, value);
        }
    }

    fn apply_cur_mat<F: Fn(Matrix) -> Matrix>(&mut self, apply: F) {
        match self.mtx_mode {
            MatrixMode::Proj => self.cur_proj = apply(self.cur_proj),
            MatrixMode::Pos => self.cur_pos = apply(self.cur_pos),
            MatrixMode::PosVec => {
                self.cur_pos = apply(self.cur_pos);
                self.cur_vec = apply(self.cur_vec);
            },
            MatrixMode::Texture => self.cur_tex = apply(self.cur_tex),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GeometryCommand {
    Unimplemented = 0x00,
    MtxMode = 0x10,
    MtxPop = 0x12,
    MtxIdentity = 0x15,
    MtxMult4x4 = 0x18,
    PolygonAttr = 0x29,
    TexImageParam = 0x2A,
    SwapBuffers = 0x50,
    Viewport = 0x60,
}

impl GeometryCommand {
    fn from_addr(addr: u32) -> Self {
        use GeometryCommand::*;
        match addr {
            0x440 => MtxMode,
            0x448 => MtxPop,
            0x454 => MtxIdentity,
            0x460 => MtxMult4x4,
            0x4A4 => PolygonAttr,
            0x4A8 => TexImageParam,
            0x540 => SwapBuffers,
            0x580 => Viewport,
            _ => { warn!("Unimplemented Geometry Command Address 0x{:X}", addr); Unimplemented },
        }
    }

    fn exec_time(&self) -> usize {
        use GeometryCommand::*;
        match *self {
            Unimplemented => 0,
            MtxMode => 0,
            MtxPop => 35,
            MtxIdentity => 18,
            MtxMult4x4 => 19,
            PolygonAttr => 0,
            TexImageParam => 0,
            SwapBuffers => 0,
            Viewport => 0,
        }
    }

    fn num_params(&self) -> usize {
        use GeometryCommand::*;
        match *self {
            Unimplemented => 0,
            MtxMode => 1,
            MtxPop => 1,
            MtxIdentity => 0,
            MtxMult4x4 => 16,
            PolygonAttr => 1,
            TexImageParam => 1,
            SwapBuffers => 1,
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

fn create_fixed_point(val: u32) -> simba::scalar::FixedI32::<fixed::types::extra::U12>{
    simba::scalar::FixedI32::<fixed::types::extra::U12>(
        fixed::FixedI32::<fixed::types::extra::U12>::from_bits(val as i32)
    )
}
