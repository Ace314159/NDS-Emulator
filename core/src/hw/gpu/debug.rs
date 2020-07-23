use super::GPU;

impl GPU {
    pub fn render_palettes<>(palettes: &Vec<u16>) -> (Vec<u16>, usize, usize) {
        let palettes_size = 16;
        let size = palettes_size * 8;
        let mut pixels = vec![0; size * size];
        for palette_y in 0..palettes_size {
            for palette_x in 0..palettes_size {
                let palette_num = palette_y * palettes_size + palette_x;
                let start_i = (palette_y * size + palette_x) * 8;
                for y in 0..8 {
                    for x in 0..8 {
                        pixels[start_i + y * size + x] = palettes[palette_num] | 0x8000;
                    }
                }
            }
        }
        (pixels, size, size)
    }
}
