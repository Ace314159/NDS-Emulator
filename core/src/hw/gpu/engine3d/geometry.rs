use super::Engine3D;
use super::math::{FixedPoint, Vec4, Matrix};
use super::registers::*;


impl Engine3D {
    fn push_geometry_command(&mut self, command: GeometryCommand, param: u32) {
        let entry = GeometryCommandEntry::new(command, param);
        self.gxfifo.push_back(entry);
        self.bus_stalled = self.gxfifo.len() >= Engine3D::FIFO_LEN;
    }

    pub fn exec_command(&mut self, command_entry: GeometryCommandEntry) {
        self.gxstat.geometry_engine_busy = false;
        self.bus_stalled = false;
        self.params.push(command_entry.param);
        if self.params.len() < command_entry.command.num_params() {
            if self.params.len() > 1 { assert_eq!(self.prev_command, command_entry.command) }
            self.prev_command = command_entry.command;
            return
        }

        use GeometryCommand::*;
        let param = command_entry.param;
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
                        self.calc_clip_mat();
                    },
                    MatrixMode::Pos | MatrixMode::PosVec => {
                        self.pos_vec_stack_sp = (self.pos_vec_stack_sp as i8 - offset) as u8;
                        assert!(self.pos_vec_stack_sp < 31);
                        self.cur_pos = self.pos_stack[self.pos_vec_stack_sp as usize];
                        self.calc_clip_mat();
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
                        self.calc_clip_mat();
                    },
                    MatrixMode::Pos | MatrixMode::PosVec => {
                        assert!(index <= 31);
                        self.cur_pos = self.pos_stack[index as usize];
                        self.calc_clip_mat();
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
            Color => self.color = self::Color::from(param as u16),
            Normal => self.calc_lighting(
                FixedPoint::from_frac9(((param >> 0) & 0x3FF) as u16),
                FixedPoint::from_frac9(((param >> 10) & 0x3FF) as u16),
                FixedPoint::from_frac9(((param >> 20) & 0x3FF) as u16),
            ),
            TexCoord => {
                self.raw_tex_coord = [
                    (self.params[0] >> 0) as u16 as i16,
                    (self.params[0] >> 16) as u16 as i16,
                ];
                self.tex_coord = self.raw_tex_coord;
                self.transform_tex_coord(TexCoordTransformationMode::TexCoord, None);
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
                self.prev_pos[0] + FixedPoint::from_frac12(((((param >> 0 & 0x3FF) << 6) as u16 as i16) >> 6) as i32),
                self.prev_pos[1] + FixedPoint::from_frac12(((((param >> 10 & 0x3FF) << 6) as u16 as i16) >> 6) as i32),
                self.prev_pos[2] + FixedPoint::from_frac12(((((param >> 20 & 0x3FF) << 6) as u16 as i16) >> 6) as i32),
            ),
            PolygonAttr => self.polygon_attrs.write(param),
            TexImageParam => self.tex_params.write(param),
            PlttBase => self.palette_base = ((self.params[0] & 0xFFF) as usize) * 16,
            DifAmb => if self.material.set_dif_amb(param) {
                self.color = super::Color::new5(
                    self.material.diffuse[0] as u8,
                    self.material.diffuse[1] as u8,
                    self.material.diffuse[2] as u8,
                );
            },
            SpeEmi => self.material.set_spe_emi(param),
            LightVector => self.lights[(param >> 30 & 0x3) as usize].direction = self.cur_vec * [
                FixedPoint::from_frac9(((param >> 0) & 0x3FF) as u16),
                FixedPoint::from_frac9(((param >> 10) & 0x3FF) as u16),
                FixedPoint::from_frac9(((param >> 20) & 0x3FF) as u16),
            ],
            LightColor => self.lights[(param >> 30 & 0x3) as usize].color = [
                (param >> 0 & 0x1F) as i32,
                (param >> 5 & 0x1F) as i32,
                (param >> 10 & 0x1F) as i32,
            ],
            Shininess => for (i, word) in self.params.iter().enumerate() {
                for byte in 0..4 {
                    self.material.shininess[i * 4 + byte] = (*word >> (8 * byte)) as u8 as i8;
                }
            },
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
    }

    pub fn write_geometry_fifo(&mut self, value: u32) {
        if self.packed_commands == 0 {
            if value == 0 {
                return
            }
            self.packed_commands = value;
            self.cur_command = GeometryCommand::from_byte(self.packed_commands as u8);
            self.num_params = self.cur_command.num_params();
            self.params_processed = 0;
            if self.num_params > 0 { return }
        } else { self.params_processed += 1 }

        while self.packed_commands != 0 {
            if self.cur_command != GeometryCommand::NOP {
                self.push_geometry_command(self.cur_command, value);
            }

            assert!(self.params_processed <= self.num_params);
            if self.params_processed == self.num_params {
                self.packed_commands >>= 8;
                if self.packed_commands != 0 {
                    self.cur_command = GeometryCommand::from_byte(self.packed_commands as u8);
                    self.num_params = self.cur_command.num_params();
                    self.params_processed = 0;
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
            MatrixMode::Proj => { apply(&mut self.cur_proj, &self.params); self.calc_clip_mat(); },
            MatrixMode::Pos => { apply(&mut self.cur_pos, &self.params); self.calc_clip_mat(); },
            MatrixMode::PosVec => {
                apply(&mut self.cur_pos, &self.params);
                self.calc_clip_mat();
                if also_to_vec { apply(&mut self.cur_vec, &self.params) }
            },
            MatrixMode::Texture => apply(&mut self.cur_tex, &self.params),
        }
    }

    fn calc_clip_mat(&mut self) {
        self.clip_mat = self.cur_pos * self.cur_proj;
    }

    fn calc_lighting(&mut self, x: FixedPoint, y: FixedPoint, z: FixedPoint) {
        self.transform_tex_coord(TexCoordTransformationMode::Normal, Some([x, y, z]));
        let normal = self.cur_vec * [x, y, z];
        let line_of_sight = [FixedPoint::zero(), FixedPoint::zero(), -FixedPoint::one()];
        let mut final_color = self.material.emission;
        for (light_i, enabled) in self.polygon_attrs_latch.lights_enabled.iter().enumerate() {
            if !enabled { continue }
            let light = &self.lights[light_i];
            let diffuse_lvl = -FixedPoint::from_mul(
                light.direction[0] * normal[0] + light.direction[1] * normal[1] + light.direction[2] * normal[2]
            ).raw() >> 4; // Convert to 8 frac
            // TODO: Use clamp
            let diffuse_lvl = if diffuse_lvl < 0 { 0 } else if diffuse_lvl > 0xFF { 0xFF } else { diffuse_lvl };

            let half_vector = [
                FixedPoint::from_frac12((light.direction[0] + line_of_sight[0]).raw() / 2),
                FixedPoint::from_frac12((light.direction[1] + line_of_sight[1]).raw() / 2),
                FixedPoint::from_frac12((light.direction[2] + line_of_sight[2]).raw() / 2),
            ];
            let shininess_lvl = -FixedPoint::from_mul(
                half_vector[0] * normal[0] + half_vector[1] * normal[1] + half_vector[2] * normal[2]
            ).raw() >> 4; // Convert to 8 frac
            let shininess_lvl = if shininess_lvl < 0 { 0 } else if shininess_lvl > 0xFF {
                (0x100 - shininess_lvl) & 0xFF // Mirroring
            } else { shininess_lvl };
            let shininess_lvl = ((2 * shininess_lvl * shininess_lvl) >> 8) - 0x100; // 0x100 = 1 in 8 frac
            let shininess_lvl = if shininess_lvl < 0 { 0 } else { shininess_lvl };

            let shininess_lvl = if self.material.use_shininess_table {
                self.material.shininess[(shininess_lvl as usize) / 2] as i32
            } else { shininess_lvl };

            for i in 0..3 {
                final_color[i] += (self.material.specular[i] * light.color[i] * shininess_lvl) >> 13;
                final_color[i] += (self.material.diffuse[i] * light.color[i] * diffuse_lvl) >> 13;
                final_color[i] += (self.material.ambient[i] * light.color[i]) >> 5;
            }
        }
        self.color = Color::new5(
            if final_color[0] > 0x1F { 0x1F } else { final_color[0] } as u8,
            if final_color[1] > 0x1F { 0x1F } else { final_color[1] } as u8,
            if final_color[2] > 0x1F { 0x1F } else { final_color[2] } as u8,
        );
    }

    // Use Const Generics
    fn transform_tex_coord(&mut self, transformation_mode: TexCoordTransformationMode, normal: Option<[FixedPoint; 3]>) {
        if self.tex_params.coord_transformation_mode != transformation_mode { return }
        let s = self.raw_tex_coord[0] as i32;
        let t = self.raw_tex_coord[1] as i32;
        let m = &self.cur_tex;
        let normal = normal.unwrap_or_else(||[FixedPoint::zero(), FixedPoint::zero(), FixedPoint::zero()]);
        self.tex_coord = match self.tex_params.coord_transformation_mode {
            TexCoordTransformationMode::None => self.raw_tex_coord,
            TexCoordTransformationMode::TexCoord => [
                ((s * m[0].raw() + t * m[4].raw() + m[8].raw() + m[12].raw()) >> 12) as i16,
                ((s * m[1].raw() + t * m[5].raw() + m[9].raw() + m[13].raw()) >> 12) as i16,
            ],
            TexCoordTransformationMode::Normal => [
                (((normal[0] * m[0] + normal[1] * m[4] + normal[2] * m[8]) >> 24) + s as i64) as i16,
                (((normal[0] * m[1] + normal[1] * m[5] + normal[2] * m[9]) >> 24) + t as i64) as i16,
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
        let clip_coords = self.clip_mat * vertex_pos;

        self.transform_tex_coord(TexCoordTransformationMode::Vertex, None);
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
        // Face Culling
        let a = (
            self.cur_poly_verts[0].clip_coords[0] - self.cur_poly_verts[1].clip_coords[0],
            self.cur_poly_verts[0].clip_coords[1] - self.cur_poly_verts[1].clip_coords[1],
            self.cur_poly_verts[0].clip_coords[3] - self.cur_poly_verts[1].clip_coords[3],
        );
        let b = (
            self.cur_poly_verts[2].clip_coords[0] - self.cur_poly_verts[1].clip_coords[0],
            self.cur_poly_verts[2].clip_coords[1] - self.cur_poly_verts[1].clip_coords[1],
            self.cur_poly_verts[2].clip_coords[3] - self.cur_poly_verts[1].clip_coords[3],
        );
        let mut normal = (
            ((a.1 * b.2) as i64 - (a.2 * b.1) as i64),
            ((a.2 * b.0) as i64 - (a.0 * b.2) as i64),
            ((a.0 * b.1) as i64 - (a.1 * b.0) as i64),
        );
        while (normal.0 >> 31) ^ (normal.1 >> 63) != 0 || (normal.1 >> 31) ^ (normal.1 >> 63) != 0 ||
            (normal.2 >> 31) ^ (normal.2 >> 63) != 0 {
            normal.0 >>= 4;
            normal.1 >>= 4;
            normal.2 >>= 4;
        }
        let vert = &self.cur_poly_verts[0].clip_coords;
        let dot = normal.0 * vert[0].raw64() + normal.1 * vert[1].raw64() + normal.2 * vert[3].raw64();

        let (is_front, should_render) = match dot {
            0 => { info!("Not Drawing Line"); (true, false) }, // TODO: Line
            _ if dot < 0 => (true, self.polygon_attrs_latch.render_front), // Front
            _ if dot > 0 => (false, self.polygon_attrs_latch.render_back), // Back
            _ => unreachable!(),
        };
        if !should_render { self.cur_poly_verts.clear(); return }

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
            is_front,
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
                    prev_vertex, cur_vertex);
                new_vert_i += 1;
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
                    prev_vertex, cur_vertex));
            }
        }
    }

    fn find_intersection(&self, coord_i: usize, positive: bool, inside: &Vertex, out: &Vertex) -> Vertex {
        let plane_factor = if positive { 1 } else { -1 };
        let factor_numer = inside.clip_coords[3].raw64() - plane_factor * inside.clip_coords[coord_i].raw64();
        let factor_denom = factor_numer - (out.clip_coords[3].raw64() - plane_factor * out.clip_coords[coord_i].raw64());
        
        let interpolate = |inside, out| inside + (out - inside) * factor_numer / factor_denom;
        let calc_coord = |i, new_w: FixedPoint| FixedPoint::from_frac12(
            if coord_i == i { plane_factor * new_w.raw64() }
            else { interpolate(inside.clip_coords[i].raw64(), out.clip_coords[i].raw64()) } as i32
        );
        let new_w = calc_coord(3, FixedPoint::zero());

        Vertex {
            clip_coords: Vec4::new(
                calc_coord(0, new_w),
                calc_coord(1, new_w),
                calc_coord(2, new_w),
                new_w,
            ),
            screen_coords: [0, 0], // Calcluated after
            z_depth: 0, // Calculated after
            color: Color::new8(
                interpolate(inside.color.r as i64, out.color.r as i64) as u8,
                interpolate(inside.color.g as i64, out.color.g as i64) as u8,
                interpolate(inside.color.b as i64, out.color.b as i64) as u8,
            ),
            tex_coord: [
                interpolate(inside.tex_coord[0] as i64, out.tex_coord[0] as i64) as i16,
                interpolate(inside.tex_coord[1] as i64, out.tex_coord[1] as i64) as i16,
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
    Shininess = 0x34,
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
            0x4D0 => Shininess,
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
            0x34 => Shininess,
            0x40 => BeginVtxs,
            0x41 => EndVtxs,
            0x50 => SwapBuffers,
            0x60 => Viewport,
            _ => { warn!("Unimplemented Geometry Command Byte: 0x{:X}", value); Unimplemented },
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
            Shininess => 32,
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

#[derive(Clone, Copy)]
pub struct Light {
    direction: [FixedPoint; 3],
    color: [i32; 3],
}

impl Light {
    pub fn new() -> Self {
        Light {
            direction: [FixedPoint::zero(), FixedPoint::zero(), FixedPoint::zero()],
            color: [0, 0, 0],
        }
    }
}

pub struct Material {
    diffuse: [i32; 3],
    ambient: [i32; 3],
    specular: [i32; 3],
    emission: [i32; 3],
    shininess: [i8; 128],
    use_shininess_table: bool,
}

impl Material {
    pub fn new() -> Self {
        Material {
            diffuse: [0, 0, 0],
            ambient: [0, 0, 0],
            specular: [0, 0, 0],
            emission: [0, 0, 0],
            shininess: [0; 128],
            use_shininess_table: false,
        }
    }

    pub fn set_dif_amb(&mut self, val: u32) -> bool {
        self.diffuse = [
            (val >> 0 & 0x1F) as i32,
            (val >> 5 & 0x1F) as i32,
            (val >> 10 & 0x1F) as i32,
        ];
        self.ambient = [
            (val >> (16 + 0) & 0x1F) as i32,
            (val >> (16 + 5) & 0x1F) as i32,
            (val >> (16 + 10) & 0x1F) as i32,
        ];
        val & 0x8000 != 0
    }

    pub fn set_spe_emi(&mut self, val: u32) {
        self.specular = [
            (val >> 0 & 0x1F) as i32,
            (val >> 5 & 0x1F) as i32,
            (val >> 10 & 0x1F) as i32,
        ];
        self.use_shininess_table = val & 0x8000 != 0;
        self.emission = [
            (val >> (16 + 0) & 0x1F) as i32,
            (val >> (16 + 5) & 0x1F) as i32,
            (val >> (16 + 10) & 0x1F) as i32,
        ];
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl From<u16> for Color {
    fn from(value: u16) -> Self {
        Color::new5(
            ((value >> 0) & 0x1F) as u8,
            ((value >> 5) & 0x1F) as u8,
            ((value >> 10) & 0x1F) as u8,
        )
    }
}
impl Color {
    // Expands 5 bit components to internal 6-bit
    pub fn new5(r: u8, g: u8, b: u8) -> Self {
        Color::new6(
            if r == 0 { 0 } else { r * 2 + 1},
            if g == 0 { 0 } else { g * 2 + 1},
            if b == 0 { 0 } else { b * 2 + 1},
        )
    }

    // Expands 6 bit components to 8 bits for interpolation
    pub fn new6(r: u8, g: u8, b: u8) -> Self {
        Color {
            r: if r == 0 { 0 } else { (r * 2 + 1) * 2 + 1 },
            g: if g == 0 { 0 } else { (g * 2 + 1) * 2 + 1 },
            b: if b == 0 { 0 } else { (b * 2 + 1) * 2 + 1 },
        }
    }

    pub fn new8(r: u8, g: u8, b: u8) -> Self {
        Color {
            r,
            g,
            b,
        }
    }

    // 8 bit components reduced to 5 bit
    pub fn as_u16(&self) -> u16 {
        (self.b as u16 >> 3) << 10 | (self.g as u16 >> 3) << 5 | (self.r as u16 >> 3) << 0
    }

    pub fn r5(&self) -> u8 { self.r >> 3 }
    pub fn g5(&self) -> u8 { self.g >> 3 }
    pub fn b5(&self) -> u8 { self.b >> 3 }
    pub fn r6(&self) -> u8 { self.r >> 2 }
    pub fn g6(&self) -> u8 { self.g >> 2 }
    pub fn b6(&self) -> u8 { self.b >> 2 }
    pub fn r8(&self) -> u8 { self.r >> 0 }
    pub fn g8(&self) -> u8 { self.g >> 0 }
    pub fn b8(&self) -> u8 { self.b >> 0 }
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
            color: Color::new8(0, 0, 0),
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
    pub is_front: bool,
}
