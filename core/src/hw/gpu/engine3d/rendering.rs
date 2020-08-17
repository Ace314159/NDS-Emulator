use super::{GPU, Engine3D};

impl Engine3D {
    pub fn get_line(&self, vcount: u16, line: &mut [u16; GPU::WIDTH]) {
        for (i, pixel) in line.iter_mut().enumerate() {
            *pixel = self.pixels[vcount as usize * GPU::WIDTH + i]
        }
    }

    pub fn render(&mut self) {
        // TODO: Add 392 cycle delay after VBlank starts
        if !self.rendering { return }
        // TODO: Actually Render
        for pixel in self.pixels.iter_mut() {
            *pixel = self.clear_color.color()
        }
        self.gxstat.geometry_engine_busy = false;
        self.rendering = false;
    }
}