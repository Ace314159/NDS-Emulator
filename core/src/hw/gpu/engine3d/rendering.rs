use super::{GPU, Color, geometry::{Polygon, Vertex}, Engine3D, super::VRAM, TextureFormat, registers::PolygonMode};

impl Engine3D {
    pub fn pixels(&self) -> &Vec<u16> {
        &self.pixels
    }

    pub fn copy_line(&self, vcount: u16, line: &mut [u16; GPU::WIDTH]) {
        for (i, pixel) in line.iter_mut().enumerate() {
            *pixel = self.pixels[vcount as usize * GPU::WIDTH + i]
        }
    }

    pub fn render(&mut self, vram: &VRAM) {
        if !self.polygons_submitted { return }
        // TODO: Add more accurate interpolation
        // TODO: Optimize
        for (i, pixel) in self.pixels.iter_mut().enumerate() {
            *pixel = self.clear_color.color();
            self.depth_buffer[i] = self.clear_depth.depth();
        }

        assert!(!self.frame_params.w_buffer); // TODO: Implement W-Buffer
        // TODO: Account for special cases
        // TODO: Remove with const generics
        fn eq_depth_test(cur_depth: u32, new_depth: u32) -> bool {
            new_depth >= cur_depth - 0x200 && new_depth <= cur_depth + 0x200
        }
        fn lt_depth_test(cur_depth: u32, new_depth: u32) -> bool { new_depth < cur_depth }

        
        for polygon in self.polygons.iter() {
            // TODO: Implement perspective correction
            // TODO: Implement translucency
            // TODO: Remove with const generics
            fn combine_colors5<F>(color_a: Color, color_b: Color, f: F) -> Color where F: Fn(u16, u16) -> u16 {
                Color::new5(
                    f(color_a.r5() as u16, color_b.r5() as u16) as u8,
                    f(color_a.g5() as u16, color_b.g5() as u16) as u8,
                    f(color_a.b5() as u16, color_b.b5() as u16) as u8,
                )
            };
            fn blend_tex<F>(tex_color: Option<Color>, vert_color: Color, f: F) -> Color where F: Fn(u16, u16) -> u16 {
                if let Some(tex_color) = tex_color {
                    Color::new6(
                        f(tex_color.r6() as u16, vert_color.r6() as u16) as u8,
                        f(tex_color.g6() as u16, vert_color.g6() as u16) as u8,
                        f(tex_color.b6() as u16, vert_color.b6() as u16) as u8,
                    )
                } else {
                    vert_color
                }
            };

            let disp3dcnt = &self.disp3dcnt;
            let toon_table = &self.toon_table;
            let blend = |vert_color, s: i32, t: i32| {
                let vram_offset = polygon.tex_params.vram_offset;
                let pal_offset = polygon.palette_base;
                let size = (polygon.tex_params.size_s as u32, polygon.tex_params.size_t as u32);
                let size_shift = (polygon.tex_params.size_s_shift, polygon.tex_params.size_t_shift);
                let mask = (size.0 - 1, size.1 - 1);
                // TODO: Avoid code repitition
                let s = if polygon.tex_params.repeat_s {
                    let (original_s, mask) = (s as u32, mask.0 as u32);
                    let s = original_s & mask;
                    if polygon.tex_params.flip_s && (original_s >> size_shift.0) % 2 == 1 { s ^ mask } else { s }
                // TODO: Replace with clamp
                } else if s < 0 { 0 } else if s as u32 > size.0 { mask.0 } else { s as u32 } as usize;
                let t = if polygon.tex_params.repeat_t {
                    let (original_t, mask) = (t as u32, mask.1 as u32);
                    let t = original_t & mask;
                    if polygon.tex_params.flip_t && (original_t >> size_shift.1) % 2 == 1 { t ^ mask } else { t }
                // TODO: Replace with clamp
                } else if t < 0 { 0 } else if t as u32 > size.1 { mask.1 } else { t as u32 } as usize;
                let texel = t * polygon.tex_params.size_s + s;
                let tex_color = match polygon.tex_params.format {
                    TextureFormat::NoTexture => None,
                    TextureFormat::A3I5 => Some({
                        // TODO: Use alpha bits
                        let byte = vram.get_textures::<u8>(vram_offset + texel);
                        let palette_color = byte & 0x1F;
                        Color::from(vram.get_textures_pal::<u16>(pal_offset + 2 * palette_color as usize))
                    }),
                    TextureFormat::Palette4 => Some({
                        let palette_color = vram.get_textures::<u8>(vram_offset + texel / 4) >> 2 * (texel % 4) & 0x3;
                        Color::from(vram.get_textures_pal::<u16>(pal_offset / 2 + 2 * palette_color as usize))
                    }),
                    TextureFormat::Palette16 => Some({
                        let palette_color = vram.get_textures::<u8>(vram_offset + texel / 2) >> 4 * (texel % 2) & 0xF;
                        Color::from(vram.get_textures_pal::<u16>(pal_offset + 2 * palette_color as usize))
                    }),
                    TextureFormat::Compressed => Some({
                        let num_blocks_row = polygon.tex_params.size_s / 4;
                        let block_start_addr = t / 4 * num_blocks_row + s / 4;
                        let base_addr = vram_offset + 4 * block_start_addr;
                        let te = vram.get_textures::<u8>(base_addr + t % 4);
                        let texel_val = te >> 2 * (s % 4) & 0x3;
                         // TODO: Check behavior and optimize
                        assert!(base_addr / 128 / 0x400 == 0 || base_addr / 128 / 0x400 == 2);
                        let extra_palette_addr = (base_addr & 0x1_FFFF) / 2 + if base_addr < 128 * 0x400 {
                            0 // Slot 0
                        } else { 0x1000 }; // Slot 2
                        let extra_palette_info = vram.get_textures::<u16>(128 * 0x400 + extra_palette_addr);
                        let mode = (extra_palette_info >> 14) & 0x3;
                        let pal_offset = pal_offset + 4 * (extra_palette_info & 0x3FFF) as usize;
                        let color = |num: u8| Color::from(
                            vram.get_textures_pal::<u16>(pal_offset + 2 * num as usize)
                        );
                        match mode {
                            0 => match texel_val {
                                0 | 1 | 2 => color(texel_val),
                                3 => Color::new8(0, 0, 0), // TODO: Implement transparency
                                _ => unreachable!(),
                            }
                            1 => match texel_val {
                                0 | 1 => color(texel_val),
                                2 => combine_colors5(color(0), color(1), |val0, val1|
                                    (val0 + val1) / 2),
                                3 => Color::new8(0, 0, 0), // TODO: Implement transparency
                                _ => unreachable!(),
                            },
                            2 => color(texel_val),
                            3 => match texel_val {
                                0 | 1 => color(texel_val),
                                2 => combine_colors5(color(0), color(1), |val0, val1|
                                    (val0 * 5 + val1 * 3) / 8),
                                3 => combine_colors5(color(0), color(1), |val0, val1|
                                    (val0 * 3 + val1 * 5) / 8),
                                _ => unreachable!(),
                            }
                            _ => unreachable!(),
                        }
                    }),
                    TextureFormat::A5I3 => Some({
                        // TODO: Use alpha bits
                        let byte = vram.get_textures::<u8>(vram_offset + texel);
                        let palette_color = byte & 0x7;
                        Color::from(vram.get_textures_pal::<u16>(pal_offset + 2 * palette_color as usize))
                    }),
                    TextureFormat::Palette256 => Some({
                        let palette_color = vram.get_textures::<u8>(vram_offset + texel);
                        Color::from(vram.get_textures_pal::<u16>(pal_offset + 2 * palette_color as usize))
                    }),
                    TextureFormat::DirectColor => Some(Color::from(vram.get_textures::<u16>(vram_offset + 2 * texel))),
                };
                let modulation_blend = |val1, val2| ((val1 + 1) * (val2 + 1) - 1) / 64;
                match polygon.attrs.mode {
                    PolygonMode::Modulation => blend_tex(tex_color, vert_color,modulation_blend),
                    PolygonMode::ToonHighlight if disp3dcnt.highlight_shading =>
                    blend_tex(tex_color, vert_color,
                        |val1, val2| std::cmp::max(modulation_blend(val1, val2) + val2, 0x3F)
                    ),
                    PolygonMode::ToonHighlight => {
                        let toon_color = toon_table[vert_color.r5() as usize];
                        blend_tex(tex_color, toon_color, modulation_blend)
                    },
                    // TODO: Use decal blending
                    PolygonMode::Shadow => tex_color.unwrap_or_else(|| Color::new5(0, 0, 0)), 
                    _ => todo!(),
                }
            };
            // TODO: Use fixed point for interpolation
            // TODO: Fix uneven interpolation
            let depth_test = if polygon.attrs.depth_test_eq { eq_depth_test } else { lt_depth_test };
            let vertices = &self.vertices[polygon.start_vert..polygon.end_vert];
            Engine3D::render_polygon(blend, depth_test, &polygon, vertices, &mut self.pixels, &mut self.depth_buffer);
        }

        self.gxstat.geometry_engine_busy = false;
        self.vertices.clear();
        self.polygons.clear();
        self.polygons_submitted = false;
    }

    fn render_polygon<B, D>(blend: B, depth_test: D, polygon: &Polygon, vertices: &[Vertex], pixels: &mut Vec<u16>, depth_buffer: &mut Vec<u32>)
    where B: Fn(Color, i32, i32) -> Color, D: Fn(u32, u32) -> bool {
        if polygon.attrs.mode == PolygonMode::Shadow { return }
        // Find top left and bottom right vertices
        let (mut start_vert, mut end_vert) = (0, 0);
        for (i, vert) in vertices.iter().enumerate() {
            if vert.screen_coords[1] < vertices[start_vert].screen_coords[1] {
                start_vert = i;
            } else if vert.screen_coords[1] == vertices[start_vert].screen_coords[1] &&
                vert.screen_coords[0] < vertices[start_vert].screen_coords[0] {
                start_vert = i;
            }

            if vert.screen_coords[1] > vertices[end_vert].screen_coords[1] {
                end_vert = i;
            } else if vert.screen_coords[1] == vertices[end_vert].screen_coords[1] &&
                vert.screen_coords[0] > vertices[end_vert].screen_coords[0] {
                end_vert = i;
            }
        }
        let mut left_vert = start_vert;
        let mut right_vert = start_vert;
        let start_vert = start_vert; // Shadow to mark these as immutable
        let end_vert = end_vert; // Shadow to mark these as immutable

        let next = |cur| if cur == vertices.len() - 1 { 0 } else { cur + 1 };
        let prev = |cur| if cur == 0 { vertices.len() - 1 } else { cur - 1 };

        let (next_left, next_right): (Box<dyn Fn(usize) -> usize>, Box<dyn Fn(usize) -> usize>) = if polygon.is_front {
            (Box::new(next), Box::new(prev))
        } else {
            (Box::new(prev), Box::new(next))
        };
        let new_left_vert = next_left(left_vert);
        let mut left_slope = VertexSlope::from_verts(
            &vertices[left_vert],
            &vertices[new_left_vert],
        );
        let mut left_end = vertices[new_left_vert].screen_coords[1];
        left_vert = new_left_vert;
        let new_right_vert = next_right(right_vert);
        let mut right_slope = VertexSlope::from_verts(
            &vertices[right_vert],
            &vertices[new_right_vert],
        );
        let mut right_end = vertices[new_right_vert].screen_coords[1];
        right_vert = new_right_vert;

        for y in vertices[start_vert].screen_coords[1]..vertices[end_vert].screen_coords[1] {
            // While loops to skip repeated vertices from clipping
            // TODO: Should this be fixed in clipping or rendering code?
            while y == left_end {
                let new_left_vert = next_left(left_vert);
                left_slope = VertexSlope::from_verts(&vertices[left_vert], &vertices[new_left_vert]);
                left_end = vertices[new_left_vert].screen_coords[1];
                left_vert = new_left_vert;
            }
            while y == right_end {
                let new_right_vert = next_right(right_vert);
                right_slope = VertexSlope::from_verts(&vertices[right_vert],&vertices[new_right_vert]);
                right_end = vertices[new_right_vert].screen_coords[1];
                right_vert = new_right_vert;
            }
            let x_start = left_slope.next_x() as usize;
            let x_end = right_slope.next_x() as usize;
            let w_start = left_slope.next_w() as i16;
            let w_end = right_slope.next_w() as i16;
            let num_steps = x_end.wrapping_sub(x_start);
            let mut color = ColorSlope::new(
                &left_slope.next_color(),
                &right_slope.next_color(),
                num_steps,
                w_start,
                w_end,
            );
            let mut s = PerspectiveSlope::new(
                left_slope.next_s(),
                right_slope.next_s(),
                num_steps,
                w_start,
                w_end,
            );
            let mut t = PerspectiveSlope::new(
                left_slope.next_t(),
                right_slope.next_t(),
                num_steps,
                w_start,
                w_end,
            );
            let mut depth = Slope::new(
                left_slope.next_depth(),
                right_slope.next_depth(),
                num_steps,
            );

            for x in x_start..x_end {
                let depth_val = depth.next() as u32;
                if depth_test(depth_buffer[y * GPU::WIDTH + x], depth_val) {
                    depth_buffer[y * GPU::WIDTH + x] = depth_val;
                    let blended_color = blend(color.next(), s.next() as i32 >> 4, t.next() as i32 >> 4);
                    pixels[y * GPU::WIDTH + x] = 0x8000 | blended_color.as_u16();
                }
            }
        }
    }
}

struct VertexSlope {
    x: Slope,
    w: Slope,
    s: PerspectiveSlope,
    t: PerspectiveSlope,
    depth: Slope,
    color: ColorSlope,
}

impl VertexSlope {
    pub fn from_verts(start: &Vertex, end: &Vertex) -> VertexSlope {
        let num_steps = end.screen_coords[1] - start.screen_coords[1];
        // TODO: Implement w-buffer
        let w_start = start.normalized_w;
        let w_end = end.normalized_w;
        VertexSlope {
            x: Slope::new(start.screen_coords[0] as f32, end.screen_coords[0] as f32, num_steps),
            w: Slope::new(w_start as f32, w_end as f32, num_steps),
            s: PerspectiveSlope::new(start.tex_coord[0] as f32, end.tex_coord[0] as f32, num_steps, w_start, w_end),
            t: PerspectiveSlope::new(start.tex_coord[1] as f32, end.tex_coord[1] as f32, num_steps, w_start, w_end),
            depth: Slope::new(start.z_depth as f32, end.z_depth as f32, num_steps),
            color: ColorSlope::new(&start.color, &end.color, num_steps, w_start, w_end),
        }
    }

    pub fn next_x(&mut self) -> f32 {
        self.x.next()
    }

    pub fn next_w(&mut self) -> f32 {
        self.w.next()
    }

    pub fn next_s(&mut self) -> f32 {
        self.s.next()
    }

    pub fn next_t(&mut self) -> f32 {
        self.t.next()
    }

    pub fn next_depth(&mut self) -> f32 {
        self.depth.next()
    }

    pub fn next_color(&mut self) -> Color {
        self.color.next()
    }
}

struct ColorSlope {
    r: PerspectiveSlope,
    g: PerspectiveSlope,
    b: PerspectiveSlope,
}

impl ColorSlope {
    pub fn new(start_color: &Color, end_color: &Color, num_steps: usize, w_start: i16, w_end: i16) -> Self {
        ColorSlope {
            r: PerspectiveSlope::new(start_color.r8() as f32, end_color.r8() as f32, num_steps, w_start, w_end),
            g: PerspectiveSlope::new(start_color.g8() as f32, end_color.g8() as f32, num_steps, w_start, w_end),
            b: PerspectiveSlope::new(start_color.b8() as f32, end_color.b8() as f32, num_steps, w_start, w_end),
        }
    }

    pub fn next(&mut self) -> Color {
        Color::new8(
            self.r.next() as u8,
            self.g.next() as u8,
            self.b.next() as u8,
        )
    }
}

struct PerspectiveSlope {
    cur: usize,
    start: f32,
    diff: f32,
    num_steps: f32,
    w_start: f32,
    w_end: f32,
}

impl PerspectiveSlope {
    pub fn new(start: f32, end: f32, num_steps: usize, w_start: i16, w_end: i16) -> Self {
        PerspectiveSlope {
            cur: 0,
            start,
            diff: end - start,
            num_steps: num_steps as f32,
            w_start: w_start as f32,
            w_end: w_end as f32,
        }
    }

    pub fn next(&mut self) -> f32 {
        // TODO: Use linear interpolation for same w values
        let factor_fn = |cur| (cur * self.w_start) / (((self.num_steps - cur) * self.w_end) + (cur * self.w_start));
        let factor = (factor_fn)(self.cur as f32);
        self.cur += 1;
        self.start + factor * self.diff
    }
}

#[derive(Debug)]
struct Slope {
    cur: f32,
    step: f32,
}

impl Slope {
    pub fn new(start: f32, end: f32, num_steps: usize) -> Self {
        Slope {
            cur: start,
            step: (end - start) / num_steps as f32,
        }
    }

    pub fn next(&mut self) -> f32 {
        let return_val = self.cur;
        self.cur += self.step;
        return_val
    }
}


