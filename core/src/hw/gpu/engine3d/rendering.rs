use super::{GPU, Engine3D};

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
            assert_eq!(polygon.vertices.len(), 3);
            let vertices = [
                &self.vertices[polygon.vertices[0]],
                &self.vertices[polygon.vertices[1]],
                &self.vertices[polygon.vertices[2]],
            ];
            for vertex in vertices.iter() {
                let screen_pos = &vertex.screen_coords;
                // TODO: Take into account polygon_attrs alpha
                self.pixels[(GPU::HEIGHT - screen_pos[1]) * GPU::WIDTH + screen_pos[0]] = 0x8000 | vertex.color;
            }
        }

        self.gxstat.geometry_engine_busy = false;
        self.polygons_submitted = false;
    }
}
