use super::Engine3D;
use super::math::{FixedPoint, Vec4, Matrix};
use super::registers::*;


impl Engine3D {
    fn push_geometry_command(&mut self, command: GeometryCommand, param: u32) {
        //assert!(!self.bus_stalled);
        let command = GeometryCommandEntry::new(command, param);
        if self.gxfifo.len() == 0 && self.gxpipe.len() < Engine3D::PIPE_LEN { self.gxpipe.push_back(command) }
        else {
            self.gxfifo.push_back(command);
            self.bus_stalled = self.gxfifo.len() >= Engine3D::FIFO_LEN;
        }
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
        self.bus_stalled = false;
        info!("Executing Geometry Command {:?} {:?}", command_entry.command, self.params);
        match command_entry.command {
            NOP => (),
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
                    self.tex_stack[self.tex_stack_sp as usize] = self.cur_tex;
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
            MtxStore => {
                let index = param & 0x3F;
                if index == 31 { self.gxstat.mat_stack_error = true }
                match self.mtx_mode {
                    MatrixMode::Proj => {
                        assert!(index <= 1);
                        self.proj_stack[0] = self.cur_proj;
                    },
                    MatrixMode::Pos | MatrixMode::PosVec => {
                        assert!(index <= 31);
                        self.pos_stack[index as usize] = self.cur_pos;
                        self.vec_stack[index as usize] = self.cur_vec;
                    },
                    MatrixMode::Texture => {
                        assert!(index <= 31);
                        self.tex_stack[index as usize] = self.cur_tex;
                    },
                }
            },
            MtxRestore => {
                let index = param & 0x3F;
                if index == 31 { self.gxstat.mat_stack_error = true }
                match self.mtx_mode {
                    MatrixMode::Proj => {
                        assert!(index <= 1);
                        self.cur_proj = self.proj_stack[0];
                    },
                    MatrixMode::Pos | MatrixMode::PosVec => {
                        assert!(index <= 31);
                        self.cur_pos = self.pos_stack[index as usize];
                        self.cur_vec = self.vec_stack[index as usize];
                    },
                    MatrixMode::Texture => {
                        assert!(index <= 31);
                        self.cur_tex = self.tex_stack[index as usize];
                    },
                }
            },
            MtxIdentity => self.apply_cur_mat(Matrix::set_identity, true),
            MtxLoad4x4 => self.apply_cur_mat(Matrix::load4x4, true),
            MtxLoad4x3 => self.apply_cur_mat(Matrix::load4x3, true),
            MtxMult4x4 => self.apply_cur_mat(Matrix::mul4x4, true),
            MtxMult4x3 => self.apply_cur_mat(Matrix::mul4x3, true),
            MtxMult3x3 => self.apply_cur_mat(Matrix::mul3x3, true),
            MtxScale => self.apply_cur_mat(Matrix::scale, false),
            MtxTrans => self.apply_cur_mat(Matrix::translate, true),
            Color => self.color = self::Color::from(param as u16), // TODO: Expand to 6 bit RGB
            Normal => warn!("Unimplemented Normal 0x{:X}", param),
            TexCoord => {
                self.raw_tex_coord = [
                    (self.params[0] >> 0) as u16 as i16,
                    (self.params[0] >> 16) as u16 as i16,
                ];
                self.tex_coord = self.raw_tex_coord;
                self.transform_tex_coord(TexCoordTransformationMode::TexCoord);
            },
            Vtx16 => self.submit_vertex(
                FixedPoint::from_frac12((self.params[0] >> 0) as u16 as i16 as i32),
                FixedPoint::from_frac12((self.params[0] >> 16) as u16 as i16 as i32),
                FixedPoint::from_frac12((self.params[1] >> 0) as u16 as i16 as i32),
            ),
            Vtx10 => self.submit_vertex(
                FixedPoint::from_frac6(((self.params[0] >> 0) & 0x3FF) as u16),
                FixedPoint::from_frac6(((self.params[0] >> 10) & 0x3FF) as u16),
                FixedPoint::from_frac6(((self.params[0] >> 20) & 0x3FF) as u16),
            ),
            VtxXY => self.submit_vertex(
                FixedPoint::from_frac12((self.params[0] >> 0) as u16 as i16 as i32),
                FixedPoint::from_frac12((self.params[0] >> 16) as u16 as i16 as i32),
                self.prev_pos[2],
            ),
            VtxXZ => self.submit_vertex(
                FixedPoint::from_frac12((self.params[0] >> 0) as u16 as i16 as i32),
                self.prev_pos[1],
                FixedPoint::from_frac12((self.params[0] >> 16) as u16 as i16 as i32),
            ),
            VtxYZ => self.submit_vertex(
                self.prev_pos[0],
                FixedPoint::from_frac12((self.params[0] >> 0) as u16 as i16 as i32),
                FixedPoint::from_frac12((self.params[0] >> 16) as u16 as i16 as i32),
            ),
            VtxDiff => self.submit_vertex(
                self.prev_pos[0] + FixedPoint::from_frac9(((self.params[0] >> 0) & 0x3FF) as u16),
                self.prev_pos[1] + FixedPoint::from_frac9(((self.params[0] >> 10) & 0x3FF) as u16),
                self.prev_pos[2] + FixedPoint::from_frac9(((self.params[0] >> 20) & 0x3FF) as u16),
            ),
            PolygonAttr => self.polygon_attrs.write(param),
            TexImageParam => self.tex_params.write(param),
            PlttBase => self.palette_base = ((self.params[0] & 0xFFF) as usize) * 16,
            DifAmb => warn!("Unimplemented Dif Amb 0x{:X}", param),
            SpeEmi => warn!("Unimplemented Spe Emi 0x{:X}", param),
            LightVector => warn!("Unimplemented Light Vector 0x{:X}", param),
            LightColor => warn!("Unimplemented Light Color 0x{:X}", param),
            BeginVtxs => {
                self.cur_poly_verts.clear();
                self.swap_verts = false;
                self.polygon_attrs_latch = self.polygon_attrs.clone();
                self.vertex_primitive = VertexPrimitive::from(param & 0x3);
            },
            EndVtxs => (), // Does Nothing
            SwapBuffers => {
                self.next_frame_params = self.frame_params;
                self.next_frame_params.write(param);
                self.polygons_submitted = true;
                self.gxstat.geometry_engine_busy = true; // Keep busy until VBlank
            },
            Viewport => self.viewport.write(param),
            Unimplemented => (),
        }
        self.params.clear();
        self.cycles_ahead -= command_entry.command.exec_time() as i32;
    }

    pub fn write_geometry_fifo(&mut self, value: u32) {
        if self.packed_commands.is_empty() {
            let mut commands = value;
            for _ in 0..4 {
                self.packed_commands.push_back(GeometryCommand::from_byte(commands as u8));
                commands >>= 8;
            }
            self.num_params = self.packed_commands.front().unwrap().num_params();
            self.params_processed = 0;
            if self.num_params > 0 { return }
            if value == 0 {
                self.push_geometry_command(GeometryCommand::NOP, 0);
                self.packed_commands.clear();
                return
            }
        } else { self.params_processed += 1 }

        while let Some(command) = self.packed_commands.front() {
            if command != &GeometryCommand::NOP {
                let command = command.clone();
                self.push_geometry_command(command, value);
            }

            assert!(self.params_processed <= self.num_params);
            if self.params_processed == self.num_params {
                self.packed_commands.pop_front().unwrap();
                if let Some(command) = self.packed_commands.front() {
                    self.params_processed = 0;
                    self.num_params = command.num_params();
                    if self.num_params > 0 { break }
                } else { break }
            } else { break }
        }
    }

    pub fn write_geometry_command(&mut self, addr: u32, value: u32) {
        let command = GeometryCommand::from_addr(addr & 0xFFF);
        if command != GeometryCommand::Unimplemented {
            self.push_geometry_command(command, value);
        }
    }

    fn apply_cur_mat<F: Fn(&mut Matrix, &Vec<u32>)>(&mut self, apply: F, also_to_vec: bool) {
        match self.mtx_mode {
            MatrixMode::Proj => apply(&mut self.cur_proj, &self.params),
            MatrixMode::Pos => apply(&mut self.cur_pos, &self.params),
            MatrixMode::PosVec => {
                apply(&mut self.cur_pos, &self.params);
                if also_to_vec { apply(&mut self.cur_vec, &self.params) }
            },
            MatrixMode::Texture => apply(&mut self.cur_tex, &self.params),
        }
    }

    // Use Const Generics
    fn transform_tex_coord(&mut self, transformation_mode: TexCoordTransformationMode) {
        if self.tex_params.coord_transformation_mode != transformation_mode { return }
        let s = self.raw_tex_coord[0] as i32;
        let t = self.raw_tex_coord[1] as i32;
        let m = self.cur_tex.elems();
        self.tex_coord = match self.tex_params.coord_transformation_mode {
            TexCoordTransformationMode::None => self.raw_tex_coord,
            TexCoordTransformationMode::TexCoord => [
                ((s * m[0].raw() + t * m[4].raw() + m[8].raw() + m[12].raw()) >> 12) as i16,
                ((s * m[1].raw() + t * m[5].raw() + m[9].raw() + m[13].raw()) >> 12) as i16,
            ],
            TexCoordTransformationMode::Vertex => [
                (((self.prev_pos[0] * m[0] + self.prev_pos[1] * m[4] + self.prev_pos[2] * m[8]) >> 24) + s as i64) as i16,
                (((self.prev_pos[0] * m[1] + self.prev_pos[1] * m[5] + self.prev_pos[2] * m[9]) >> 24) + t as i64) as i16,
            ],
        };
    }

    fn submit_vertex(&mut self, x: FixedPoint, y: FixedPoint, z: FixedPoint) {
        self.prev_pos = [x, y, z];
        let vertex_pos = Vec4::new(x, y, z, FixedPoint::one());
        let clip_coords = self.cur_pos * self.cur_proj * vertex_pos;

        self.transform_tex_coord(TexCoordTransformationMode::Vertex);
        self.cur_poly_verts.push(Vertex {
            clip_coords,
            screen_coords: [0, 0], // Temp - Calculated after clipping
            z_depth: 0, // Temp - Calculated after clipping
            color: self.color,
            tex_coord: self.tex_coord,
        });
        match self.vertex_primitive {
            VertexPrimitive::Triangles => {
                if self.cur_poly_verts.len() == 3 {
                    self.submit_polygon();
                }
            },
            VertexPrimitive::Quad => {
                if self.cur_poly_verts.len() == 4 {
                    self.submit_polygon();
                }
            }
            VertexPrimitive::TriangleStrips => {
                if self.cur_poly_verts.len() == 3 {
                    let new_vert0 = self.cur_poly_verts[1];
                    let new_vert1 = self.cur_poly_verts[2];
                    if self.swap_verts { self.cur_poly_verts.swap(1, 2) }
                    self.submit_polygon();
                    self.cur_poly_verts.push(new_vert0);
                    self.cur_poly_verts.push(new_vert1);
                    self.swap_verts = !self.swap_verts;
                }
            },
            VertexPrimitive::QuadStrips => {
                if self.cur_poly_verts.len() == 4 {
                    let new_vert0 = self.cur_poly_verts[2];
                    let new_vert1 = self.cur_poly_verts[3];
                    self.cur_poly_verts.swap(2, 3);
                    self.submit_polygon();
                    self.cur_poly_verts.push(new_vert0);
                    self.cur_poly_verts.push(new_vert1);
                    self.swap_verts = !self.swap_verts;
                }
            },
        }
    }

    fn submit_polygon(&mut self) {
        // Clip Polygon
        self.clip_plane(2);
        self.clip_plane(1);
        self.clip_plane(0);
        if self.cur_poly_verts.len() == 0 { return }

        // TODO: Reject polygon if it doesn't fit into Vertex RAM or Polygon 
        self.polygons.push(Polygon {
            start_vert: self.vertices.len(),
            end_vert: self.vertices.len() + self.cur_poly_verts.len(),
            attrs: self.polygon_attrs_latch,
            tex_params: self.tex_params,
            palette_base: self.palette_base,
        });
        for vert in self.cur_poly_verts.drain(..) {
            let z = vert.clip_coords[2].raw() as i64;
            let w = vert.clip_coords[3].raw() as i64;
            self.vertices.push(Vertex {
                screen_coords: [
                    self.viewport.screen_x(&vert.clip_coords),
                    self.viewport.screen_y(&vert.clip_coords),
                ],
                z_depth: ((((z * 0x4000 / w) + 0x3FFF) * 0x200) & 0xFFFFFF) as u32,
                ..vert
            });
        }
    }

    fn clip_plane(&mut self, coord_i: usize) {
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
            z_depth: 0, // Calculated after
            color: Color::new(
                interpolate(inside.color.r as i32, out.color.r as i32) as u8,
                interpolate(inside.color.g as i32, out.color.g as i32) as u8,
                interpolate(inside.color.b as i32, out.color.b as i32) as u8,
            ),
            tex_coord: [
                interpolate(inside.tex_coord[0] as i32, out.tex_coord[0] as i32) as i16,
                interpolate(inside.tex_coord[1] as i32, out.tex_coord[1] as i32) as i16,
            ]
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GeometryCommand {
    NOP = 0x00,
    MtxMode = 0x10,
    MtxPush = 0x11,
    MtxPop = 0x12,
    MtxStore = 0x13,
    MtxRestore = 0x14,
    MtxIdentity = 0x15,
    MtxLoad4x4 = 0x16,
    MtxLoad4x3 = 0x17,
    MtxMult4x4 = 0x18,
    MtxMult4x3 = 0x19,
    MtxMult3x3 = 0x1A,
    MtxScale = 0x1B,
    MtxTrans = 0x1C,
    Color = 0x20,
    Normal = 0x21,
    TexCoord = 0x22,
    Vtx16 = 0x23,
    Vtx10 = 0x24,
    VtxXY = 0x25,
    VtxXZ = 0x26,
    VtxYZ = 0x27,
    VtxDiff = 0x28,
    PolygonAttr = 0x29,
    TexImageParam = 0x2A,
    PlttBase = 0x2B,
    DifAmb = 0x30,
    SpeEmi = 0x31,
    LightVector = 0x32,
    LightColor = 0x33,
    BeginVtxs = 0x40,
    EndVtxs = 0x41,
    SwapBuffers = 0x50,
    Viewport = 0x60,
    Unimplemented = 0xFF,
}

impl GeometryCommand {
    fn from_addr(addr: u32) -> Self {
        use GeometryCommand::*;
        match addr {
            0x440 => MtxMode,
            0x444 => MtxPush,
            0x448 => MtxPop,
            0x44C => MtxStore,
            0x450 => MtxRestore,
            0x454 => MtxIdentity,
            0x458 => MtxLoad4x4,
            0x45C => MtxLoad4x3,
            0x460 => MtxMult4x4,
            0x464 => MtxMult4x3,
            0x468 => MtxMult3x3,
            0x46C => MtxScale,
            0x470 => MtxTrans,
            0x480 => Color,
            0x484 => Normal,
            0x488 => TexCoord,
            0x48C => Vtx16,
            0x490 => Vtx10,
            0x494 => VtxXY,
            0x498 => VtxXZ,
            0x49C => VtxYZ,
            0x4A4 => PolygonAttr,
            0x4A8 => TexImageParam,
            0x4AC => PlttBase,
            0x4C0 => DifAmb,
            0x4C4 => SpeEmi,
            0x4C8 => LightVector,
            0x4CC => LightColor,
            0x500 => BeginVtxs,
            0x504 => EndVtxs,
            0x540 => SwapBuffers,
            0x580 => Viewport,
            _ => { warn!("Unimplemented Geometry Command Address 0x{:X}", addr); Unimplemented },
        }
    }

    fn from_byte(value: u8) -> Self {
        use GeometryCommand::*;
        match value {
            0x00 => NOP,
            0x10 => MtxMode,
            0x11 => MtxPush,
            0x12 => MtxPop,
            0x13 => MtxStore,
            0x14 => MtxRestore,
            0x15 => MtxIdentity,
            0x16 => MtxLoad4x4,
            0x17 => MtxLoad4x3,
            0x18 => MtxMult4x4,
            0x19 => MtxMult4x3,
            0x1A => MtxMult3x3,
            0x1B => MtxScale,
            0x1C => MtxTrans,
            0x20 => Color,
            0x21 => Normal,
            0x22 => TexCoord,
            0x23 => Vtx16,
            0x24 => Vtx10,
            0x25 => VtxXY,
            0x26 => VtxXZ,
            0x27 => VtxYZ,
            0x28 => VtxDiff,
            0x29 => PolygonAttr,
            0x2A => TexImageParam,
            0x2B => PlttBase,
            0x30 => DifAmb,
            0x31 => SpeEmi,
            0x32 => LightVector,
            0x33 => LightColor,
            0x40 => BeginVtxs,
            0x41 => EndVtxs,
            0x50 => SwapBuffers,
            0x60 => Viewport,
            _ => { warn!("Unimplemented Geometry Command Byte: 0x{:X}", value); Unimplemented },
        }
    }

    fn exec_time(&self) -> usize {
        use GeometryCommand::*;
        match *self {
            NOP => 0,
            MtxMode => 0,
            MtxPush => 16,
            MtxPop => 35,
            MtxStore => 17,
            MtxRestore => 36,
            MtxIdentity => 18,
            MtxLoad4x4 => 34,
            MtxLoad4x3 => 30,
            MtxMult4x4 => 19, // TOOD: Add extra cycles for MTX_MODE 2
            MtxMult4x3 => 19, // TODO: Add extra cycles for MTX_MODE 2
            MtxMult3x3 => 19, // TODO: Add extra cycles for MTX_MODE 2
            MtxScale => 22, // TODO: Add extra cycles for MTX_MODE 2
            MtxTrans => 19, // TODO: Add extra cycles for MTX_MODE 2
            Color => 0,
            Normal => 9, // TODO: Add extra cycles depending on num of enabled lights
            TexCoord => 1,
            Vtx16 => 7,
            Vtx10 => 8,
            VtxXY => 8,
            VtxXZ => 8,
            VtxYZ => 8,
            VtxDiff => 8,
            PolygonAttr => 0,
            TexImageParam => 0,
            PlttBase => 1,
            DifAmb => 4,
            SpeEmi => 4,
            LightVector => 6,
            LightColor => 1,
            BeginVtxs => 0,
            EndVtxs => 0,
            SwapBuffers => 392,
            Viewport => 0,
            Unimplemented => 0,
        }
    }

    fn num_params(&self) -> usize {
        use GeometryCommand::*;
        match *self {
            NOP => 0,
            MtxMode => 1,
            MtxPush => 0,
            MtxPop => 1,
            MtxStore => 1,
            MtxRestore => 1,
            MtxIdentity => 0,
            MtxLoad4x4 => 16,
            MtxLoad4x3 => 12,
            MtxMult4x4 => 16,
            MtxMult4x3 => 12,
            MtxMult3x3 => 9,
            MtxScale => 3,
            MtxTrans => 3,
            Color => 1,
            Normal => 1,
            TexCoord => 1,
            Vtx16 => 2,
            Vtx10 => 1,
            VtxXY => 1,
            VtxXZ => 1,
            VtxYZ => 1,
            VtxDiff => 1,
            PolygonAttr => 1,
            TexImageParam => 1,
            PlttBase => 1,
            DifAmb => 1,
            SpeEmi => 1,
            LightVector => 1,
            LightColor => 1,
            BeginVtxs => 1,
            EndVtxs => 0,
            SwapBuffers => 1,
            Viewport => 1,
            Unimplemented => 0,
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
    pub z_depth: u32, // 24 bit depth
    pub color: Color,
    pub tex_coord: [i16; 2], // 1 + 11 + 4 fixed point
}

impl Vertex {
    pub fn new() -> Self {
        Vertex {
            clip_coords: Vec4::new(FixedPoint::zero(), FixedPoint::zero(), FixedPoint::zero(), FixedPoint::zero()),
            screen_coords: [0, 0],
            z_depth: 0,
            color: Color::new(0, 0, 0),
            tex_coord: [0, 0],
        }
    }
}

pub struct Polygon {
    pub start_vert: usize,
    pub end_vert: usize,
    pub attrs: PolygonAttributes,
    pub tex_params: TextureParams,
    pub palette_base: usize,
}
