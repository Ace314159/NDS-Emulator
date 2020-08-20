use super::Engine3D;
use super::math::{FixedPoint, Vec4, Matrix};
use super::registers::*;


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
        info!("Executing Geometry Command {:?} {:?}", command_entry.command, self.params);
        match command_entry.command {
            Unimplemented => (),
            MtxMode => self.mtx_mode = MatrixMode::from(param as u8 & 0x3),
            MtxPush => match self.mtx_mode {
                MatrixMode::Proj => {
                    self.proj_stack[self.proj_stack_sp as usize] = self.cur_proj;
                    self.proj_stack_sp += 1;
                    assert!(self.proj_stack_sp <= 1);
                },
                MatrixMode::Pos | MatrixMode::PosVec => {
                    self.pos_stack[self.pos_vec_stack_sp as usize] = self.cur_pos;
                    self.vec_stack[self.pos_vec_stack_sp as usize] = self.cur_vec;
                    self.pos_vec_stack_sp += 1;
                    assert!(self.pos_vec_stack_sp <= 31);
                },
                MatrixMode::Texture => {
                    self.cur_tex = self.tex_stack[self.tex_stack_sp as usize];
                    self.tex_stack_sp += 1;
                    assert!(self.tex_stack_sp <= 31);
                },
            },
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
            MtxIdentity => self.apply_cur_mat(Matrix::set_identity),
            MtxMult4x4 => self.apply_cur_mat(Matrix::mul4x4),
            MtxMult4x3 => self.apply_cur_mat(Matrix::mul4x3),
            MtxMult3x3 => self.apply_cur_mat(Matrix::mul3x3),
            MtxTrans => self.apply_cur_mat(Matrix::translate),
            Color => self.color = self::Color::from(param as u16), // TODO: Expand to 6 bit RGB
            Vtx16 => self.submit_vertex(
                FixedPoint::from_frac12((self.params[0] >> 0) as u16 as i16 as i32),
                FixedPoint::from_frac12((self.params[0] >> 16) as u16 as i16 as i32),
                FixedPoint::from_frac12((self.params[1] >> 0) as u16 as i16 as i32),
            ),
            PolygonAttr => self.polygon_attrs.write(param),
            TexImageParam => self.tex_params.write(param),
            BeginVtxs => {
                self.polygon_attrs_latch = self.polygon_attrs_latch.clone();
                self.vertex_primitive = VertexPrimitive::from(param & 0x3);
            },
            EndVtxs => (), // Does Nothing
            SwapBuffers => {
                self.polygons_submitted = true;
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

    fn apply_cur_mat<F: Fn(&mut Matrix, &Vec<u32>)>(&mut self, apply: F) {
        match self.mtx_mode {
            MatrixMode::Proj => apply(&mut self.cur_proj, &self.params),
            MatrixMode::Pos => apply(&mut self.cur_pos, &self.params),
            MatrixMode::PosVec => {
                apply(&mut self.cur_pos, &self.params);
                apply(&mut self.cur_vec, &self.params);
            },
            MatrixMode::Texture => apply(&mut self.cur_tex, &self.params),
        }
    }

    fn submit_vertex(&mut self, x: FixedPoint, y: FixedPoint, z: FixedPoint) {
        let vertex_pos = Vec4::new(x, y, z, FixedPoint::one());
        let clip_coords = self.cur_pos * self.cur_proj * vertex_pos;

        self.cur_poly_verts.push(Vertex {
            clip_coords,
            screen_coords: [0, 0], // Temp - Calculated after clipping
            color: self.color,
        });
        match self.vertex_primitive {
            VertexPrimitive::Triangles => {
                if self.cur_poly_verts.len() == 3 {
                    self.submit_triangle();
                }
            },
            _ => todo!(),
        }
    }

    fn submit_triangle(&mut self) {
        // Clip Triangle
        self.clip_plane(2);
        self.clip_plane(1);
        self.clip_plane(0);

        // TODO: Reject polygon if it doesn't fit into Vertex RAM or Polygon 
        let num_verts = self.cur_poly_verts.len();
        for vert in self.cur_poly_verts.drain(..) {
            self.vertices.push(Vertex {
                screen_coords: [
                    self.viewport.screen_x(&vert.clip_coords),
                    self.viewport.screen_y(&vert.clip_coords),
                ],
                ..vert
            });
        }

        let start = self.vertices.len() - num_verts;
        match num_verts {
            3 => self.polygons.push(Polygon {
                indices: [start, start + 1, start + 2],
                attrs: self.polygon_attrs_latch,
            }),
            4 => {
                self.polygons.push(Polygon {
                    indices: [start, start + 1, start + 2],
                    attrs: self.polygon_attrs_latch,
                });
                self.polygons.push(Polygon {
                    indices: [start + 2, start + 3, start],
                    attrs: self.polygon_attrs_latch,
                })
            }
            _ => todo!(),
        }
    }

    // TODO: Support Quads
    fn clip_plane(&mut self, coord_i: usize) {
        assert!(self.cur_poly_verts.len() == 3 || self.cur_poly_verts.len() == 4);
        let mut new_verts = [Vertex::new(); 10];
        let mut new_vert_i = 0;
        // Chekc positive plane
        for i in 0..self.cur_poly_verts.len() {
            let cur_vertex = &self.cur_poly_verts[i];
            let prev_index = if i == 0 { self.cur_poly_verts.len() - 1 } else { i - 1 };
            let prev_vertex = &self.cur_poly_verts[prev_index];

            // Cur Point inside positive part of plane
            if cur_vertex.clip_coords[coord_i] <= cur_vertex.clip_coords[3] {
                // TODO: Check polygon_attrs for far plane intersection
                // Prev Point outside
                if prev_vertex.clip_coords[coord_i] > prev_vertex.clip_coords[3] {
                    new_verts[new_vert_i] = self.find_intersection(coord_i, true,
                        cur_vertex, prev_vertex);
                    new_vert_i += 1;
                }
                new_verts[new_vert_i] = cur_vertex.clone();
                new_vert_i += 1;
            } else if prev_vertex.clip_coords[coord_i] <= prev_vertex.clip_coords[3] { // Prev point inside
                new_verts[new_vert_i] = self.find_intersection(coord_i, true,
                    cur_vertex, prev_vertex);
            }
        }
        self.cur_poly_verts.clear();

        // Check negative plane
        for i in 0..new_vert_i {
            let cur_vertex = &new_verts[i];
            let prev_index = if i == 0 { new_vert_i - 1 } else { i - 1 };
            let prev_vertex = &new_verts[prev_index];

            // Cur Point inside negative part of plane
            if cur_vertex.clip_coords[coord_i] >= -cur_vertex.clip_coords[3] {
                // TODO: Check polygon_attrs for far plane intersection
                // Prev Point outside
                if prev_vertex.clip_coords[coord_i] < -prev_vertex.clip_coords[3] {
                    self.cur_poly_verts.push(self.find_intersection(coord_i, false,
                        cur_vertex, prev_vertex));
                }
                self.cur_poly_verts.push(cur_vertex.clone());
            } else if prev_vertex.clip_coords[coord_i] >= -prev_vertex.clip_coords[3] { // Prev point inside
                self.cur_poly_verts.push(self.find_intersection(coord_i, false,
                    cur_vertex, prev_vertex));
            }
        }
    }

    fn find_intersection(&self, coord_i: usize, positive: bool, inside: &Vertex, out: &Vertex) -> Vertex {
        let plane_factor = if positive { 1 } else { -1 };
        let factor_numer = inside.clip_coords[3].raw() - plane_factor * inside.clip_coords[coord_i].raw();
        let factor_denom = factor_numer - (out.clip_coords[3].raw() - plane_factor * out.clip_coords[coord_i].raw());
        
        let interpolate = |inside, out| inside + (out - inside) *
            factor_numer / factor_denom;
        let calc_coord = |i| FixedPoint::from_frac12(
            interpolate(inside.clip_coords[i].raw(), out.clip_coords[i].raw()),
        );

        Vertex {
            clip_coords: Vec4::new(
                calc_coord(0),
                calc_coord(1),
                calc_coord(2),
                calc_coord(3),
            ),
            screen_coords: [0, 0], // Calcluated after
            color: Color::new(
                interpolate(inside.color.r as i32, out.color.r as i32) as u8,
                interpolate(inside.color.g as i32, out.color.g as i32) as u8,
                interpolate(inside.color.b as i32, out.color.b as i32) as u8,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GeometryCommand {
    Unimplemented = 0x00,
    MtxMode = 0x10,
    MtxPush = 0x11,
    MtxPop = 0x12,
    MtxIdentity = 0x15,
    MtxMult4x4 = 0x18,
    MtxMult4x3 = 0x19,
    MtxMult3x3 = 0x1A,
    MtxTrans = 0x1C,
    Color = 0x20,
    Vtx16 = 0x23,
    PolygonAttr = 0x29,
    BeginVtxs = 0x40,
    EndVtxs = 0x41,
    TexImageParam = 0x2A,
    SwapBuffers = 0x50,
    Viewport = 0x60,
}

impl GeometryCommand {
    fn from_addr(addr: u32) -> Self {
        use GeometryCommand::*;
        match addr {
            0x440 => MtxMode,
            0x444 => MtxPush,
            0x448 => MtxPop,
            0x454 => MtxIdentity,
            0x460 => MtxMult4x4,
            0x464 => MtxMult4x3,
            0x468 => MtxMult3x3,
            0x470 => MtxTrans,
            0x480 => Color,
            0x48C => Vtx16,
            0x4A4 => PolygonAttr,
            0x4A8 => TexImageParam,
            0x500 => BeginVtxs,
            0x504 => EndVtxs,
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
            MtxPush => 16,
            MtxPop => 35,
            MtxIdentity => 18,
            MtxMult4x4 => 19, // TOOD: Add extra cycles for MTX_MODE 2
            MtxMult4x3 => 19, // TODO: Add extra cycles for MTX_MODE 2
            MtxMult3x3 => 19, // TODO: Add extra cycles for MTX_MODE 2
            MtxTrans => 19, // TODO: Add extra cycles for MTX_MODE 2
            Color => 0,
            Vtx16 => 7,
            PolygonAttr => 0,
            TexImageParam => 0,
            BeginVtxs => 0,
            EndVtxs => 0,
            SwapBuffers => 0,
            Viewport => 0,
        }
    }

    fn num_params(&self) -> usize {
        use GeometryCommand::*;
        match *self {
            Unimplemented => 0,
            MtxMode => 1,
            MtxPush => 0,
            MtxPop => 1,
            MtxIdentity => 0,
            MtxMult4x4 => 16,
            MtxMult4x3 => 12,
            MtxMult3x3 => 9,
            MtxTrans => 3,
            Color => 1,
            Vtx16 => 2,
            PolygonAttr => 1,
            TexImageParam => 1,
            BeginVtxs => 1,
            EndVtxs => 0,
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

#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl From<u16> for Color {
    fn from(value: u16) -> Self {
        Color {
            r: ((value >> 0) & 0x1F) as u8,
            g: ((value >> 5) & 0x1F) as u8,
            b: ((value >> 10) & 0x1F) as u8,
        }
    }
}
impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Color {
            r,
            g,
            b,
        }
    }

    pub fn as_u16(&self) -> u16 {
        (self.b as u16) << 10 | (self.g as u16) << 5 | (self.r as u16) << 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub clip_coords: Vec4,
    pub screen_coords: [usize; 2],
    pub color: Color,
}

impl Vertex {
    pub fn new() -> Self {
        Vertex {
            clip_coords: Vec4::new(FixedPoint::zero(), FixedPoint::zero(), FixedPoint::zero(), FixedPoint::zero()),
            screen_coords: [0, 0],
            color: Color::new(0, 0, 0),
        }
    }
}

pub struct Polygon {
    pub indices: [usize; 3],
    pub attrs: PolygonAttributes,
}
