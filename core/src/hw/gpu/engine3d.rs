use super::GPU;

pub struct Engine3D {

}

impl Engine3D {
    pub fn new() -> Self {
        Engine3D {

        }
    }

    pub fn render_line(&self, line: &mut [u16; GPU::WIDTH]) {
        for pixel in line.iter_mut() { *pixel = 0x83F5 }
    }
}
