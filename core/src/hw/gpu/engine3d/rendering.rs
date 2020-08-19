use super::{GPU, Color, Engine3D};

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
            // TODO: Support rendering quads
            let mut vertices = [
                &self.vertices[polygon.vert_start_index + 0],
                &self.vertices[polygon.vert_start_index + 1],
                &self.vertices[polygon.vert_start_index + 2],
            ];
            vertices.sort_by_key(|vertex| (vertex.screen_coords[1], vertex.screen_coords[0]));
            let (x0, y0) = (vertices[0].screen_coords[0], vertices[0].screen_coords[1]);
            let (x1, y1) = (vertices[1].screen_coords[0], vertices[1].screen_coords[1]);
            let (x2, y2) = (vertices[2].screen_coords[0], vertices[2].screen_coords[1]);
            if y0 == y1 && y1 == y2 { continue } // All points are on the same line
            let mut sides = [
                Slope::new(x0 as f32, x1 as f32, y1 - y0),
                Slope::new(x0 as f32, x2 as f32, y2 - y0),
            ];
            let mut color_sides = [
                ColorSlope::new(vertices[0].color, vertices[1].color, y1 - y0),
                ColorSlope::new(vertices[0].color, vertices[2].color, y2 - y0),
            ];
            let pointing_right = (y1 as i32 - y0 as i32) * (x2 as i32 - x0 as i32) <
                (x1 as i32- x0 as i32 ) * (y2 as i32 - y0 as i32);
            let left_index = if pointing_right { 1 } else { 0 };
            let right_index = 1 - left_index;
            let change_index = if pointing_right { right_index } else { left_index };
            // TODO: Check if neighboring triangles are rendered properly
            for y in y0..y2 {
                if y == y1 {
                    sides[change_index] = Slope::new(x1 as f32, x2 as f32, y2 - y1);
                    color_sides[change_index] = ColorSlope::new(vertices[1].color,
                        vertices[2].color, y2 - y1);
                }
                let start_x = sides[left_index].next() as usize;
                let end_x = sides[right_index].next() as usize;
                let start_color = color_sides[left_index].next();
                let end_color = color_sides[right_index].next();
                let mut color_slope = ColorSlope::new(start_color, end_color, end_x - start_x);
                for x in start_x..end_x {
                    // TODO: Take into account alpha
                    // TODO: Use higher bit-depth for better interpolation
                    self.pixels[y * GPU::WIDTH + x] = 0x8000 | color_slope.next().as_u16();
                }
            }
        }

        self.gxstat.geometry_engine_busy = false;
        self.vertices.clear();
        self.polygons.clear();
        self.polygons_submitted = false;
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
    pub fn new(start_color: Color, end_color: Color, num_steps: usize) -> Self {
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
