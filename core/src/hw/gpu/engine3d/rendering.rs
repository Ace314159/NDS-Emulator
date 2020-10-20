use super::{GPU, Color, geometry::Vertex, Engine3D};

impl Engine3D {
    pub fn get_line(&self, vcount: u16, line: &mut [u16; GPU::WIDTH]) {
        for (i, pixel) in line.iter_mut().enumerate() {
            *pixel = self.pixels[vcount as usize * GPU::WIDTH + i]
        }
    }

    pub fn render(&mut self) {
        // TODO: Add 392 cycle delay after VBlank starts
        if !self.polygons_submitted { return }
        // TODO: Add more accurate interpolation
        // TODO: Optimize
        // TODO: Textures
        // TODO: Z-buffer
        for pixel in self.pixels.iter_mut() {
            *pixel = self.clear_color.color() 
        }

        for polygon in self.polygons.iter() {
            // TODO: Use fixed point for interpolation
            // TODO: Fix uneven interpolation
            let vertices = &self.vertices[polygon.start_vert..polygon.end_vert];
            if self.polygon_attrs_latch.render_front { Engine3D::render_polygon(true, &mut self.pixels, vertices) }
            if self.polygon_attrs_latch.render_back { Engine3D::render_polygon(false, &mut self.pixels, vertices) }
        }

        self.gxstat.geometry_engine_busy = false;
        self.vertices.clear();
        self.polygons.clear();
        self.polygons_submitted = false;
    }

    // TODO: Replace front with a const generic
    fn render_polygon(front: bool, pixels: &mut Vec<u16>, vertices: &[Vertex]) {
        assert!(vertices.len() >= 3);

        // Check if front or back side is being rendered
        let a = (
            vertices[0].screen_coords[0] as i32 - vertices[1].screen_coords[0] as i32,
            vertices[0].screen_coords[1] as i32 - vertices[1].screen_coords[1] as i32,
        );
        let b = (
            vertices[2].screen_coords[0] as i32 - vertices[1].screen_coords[0] as i32,
            vertices[2].screen_coords[1] as i32 - vertices[1].screen_coords[1] as i32,
        );
        let cross_product = a.1 * b.0 - a.0 * b.1;
        let can_draw = match cross_product {
            0 => { warn!("Not Drawing Line"); false }, // Line - TODO
            _ if cross_product < 0 => front, // Front
            _ if cross_product > 0 => !front, // Back
            _ => unreachable!(),
        };
        if !can_draw { return }
        
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

        let counterclockwise = |cur| if cur == vertices.len() - 1 { 0 } else { cur + 1 };
        let clockwise = |cur| if cur == 0 { vertices.len() - 1 } else { cur - 1 };

        let (prev_vert, next_vert): (Box<dyn Fn(usize) -> usize>, Box<dyn Fn(usize) -> usize>) = if front {
            (Box::new(counterclockwise), Box::new(clockwise))
        } else {
            (Box::new(clockwise), Box::new(counterclockwise))
        };
        let new_left_vert = prev_vert(left_vert);
        let mut left_colors = ColorSlope::new(
            &vertices[left_vert].color,
            &vertices[new_left_vert].color,
            vertices[new_left_vert].screen_coords[1] - vertices[left_vert].screen_coords[1],
        );
        let mut left_slope = Slope::from_verts(
            &vertices[left_vert],
            &vertices[new_left_vert],
        );
        let mut left_end = vertices[new_left_vert].screen_coords[1];
        left_vert = new_left_vert;
        let new_right_vert = next_vert(right_vert);
        let mut right_slope = Slope::from_verts(
            &vertices[right_vert],
            &vertices[new_right_vert],
        );
        let mut right_colors = ColorSlope::new(
            &vertices[right_vert].color,
            &vertices[new_right_vert].color,
            vertices[new_right_vert].screen_coords[1] - vertices[right_vert].screen_coords[1],
        );
        let mut right_end = vertices[new_right_vert].screen_coords[1];
        right_vert = new_right_vert;

        for y in vertices[start_vert].screen_coords[1]..vertices[end_vert].screen_coords[1] {
            // While loops to skip repeated vertices from clipping
            // TODO: Should this be fixed in clipping or rendering code?
            while y == left_end {
                let new_left_vert = prev_vert(left_vert);
                left_slope = Slope::from_verts(&vertices[left_vert], &vertices[new_left_vert]);
                left_colors = ColorSlope::new(
                    &vertices[left_vert].color,
                    &vertices[new_left_vert].color,
                    vertices[new_left_vert].screen_coords[1] - vertices[left_vert].screen_coords[1],
                );
                left_end = vertices[new_left_vert].screen_coords[1];
                left_vert = new_left_vert;
            }
            while y == right_end {
                let new_right_vert = next_vert(right_vert);
                right_slope = Slope::from_verts(&vertices[right_vert],&vertices[new_right_vert]);
                right_colors = ColorSlope::new(
                    &vertices[right_vert].color,
                    &vertices[new_right_vert].color,
                    vertices[new_right_vert].screen_coords[1] - vertices[right_vert].screen_coords[1],
                );
                right_end = vertices[new_right_vert].screen_coords[1];
                right_vert = new_right_vert;
            }
            let x_start = left_slope.next() as usize;
            let x_end = right_slope.next() as usize;
            let mut color = ColorSlope::new(
                &left_colors.next(),
                &right_colors.next(),
                x_end - x_start,
            );

            for x in x_start..x_end {
                pixels[y * GPU::WIDTH + x] = 0x8000 | color.next().as_u16();
            }
        }
    }
}

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

    pub fn from_verts(start: &Vertex, end: &Vertex) -> Self {
        Slope {
            cur: start.screen_coords[0] as f32,
            step: (end.screen_coords[0] as f32 - start.screen_coords[0] as f32) /
                (end.screen_coords[1] as f32 - start.screen_coords[1] as f32),
        }
    }

    pub fn next(&mut self) -> f32 {
        let return_val = self.cur;
        self.cur += self.step;
        return_val
    }
}

struct ColorSlope {
    r: Slope,
    g: Slope,
    b: Slope,
}

impl ColorSlope {
    pub fn new(start_color: &Color, end_color: &Color, num_steps: usize) -> Self {
        ColorSlope {
            r: Slope::new(start_color.r as f32, end_color.r as f32, num_steps),
            g: Slope::new(start_color.g as f32, end_color.g as f32, num_steps),
            b: Slope::new(start_color.b as f32, end_color.b as f32, num_steps),
        }
    }

    pub fn next(&mut self) -> Color {
        Color::new(
            self.r.next() as u8,
            self.g.next() as u8,
            self.b.next() as u8,
        )
    }
}
