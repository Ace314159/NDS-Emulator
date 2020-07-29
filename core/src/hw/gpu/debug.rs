use super::{Engine2D, GPU, VRAM};

impl GPU {
    pub fn render_palettes<F: Fn(usize) -> u16>(get_color: F, palettes_size: usize) -> (Vec<u16>, usize, usize) {
        let size = palettes_size * 8;
        let mut pixels = vec![0; size * size];
        for palette_y in 0..palettes_size {
            for palette_x in 0..palettes_size {
                let palette_num = palette_y * palettes_size + palette_x;
                let start_i = (palette_y * size + palette_x) * 8;
                for y in 0..8 {
                    for x in 0..8 {
                        pixels[start_i + y * size + x] = get_color(palette_num) | 0x8000;
                    }
                }
            }
        }
        (pixels, size, size)
    }
}

impl Engine2D {
    pub fn render_map<F: Fn(&VRAM, usize) -> u8>(&self, vram: &VRAM, get_bg: &F, bg_i: usize) -> (Vec<u16>, usize, usize) {
        let bgcnt = self.bgcnts[bg_i];
        let affine = false; // TODO: Use correct condition
        let (width, height) = match bgcnt.screen_size {
            0 => [(256, 256), (128, 128)][affine as usize],
            1 => [(512, 256), (256, 256)][affine as usize],
            2 => [(256, 512), (512, 512)][affine as usize],
            3 => [(512, 512), (1024, 1024)][affine as usize],
            _ => unreachable!(),
        };
        let mut pixels = vec![0u16; width * height];
        let tile_start_addr = self.calc_tile_start_addr(&bgcnt);
        let map_start_addr = self.calc_map_start_addr(&bgcnt);
        let bit_depth = if bgcnt.bpp8 { 8 } else { 4 }; // Also bytes per row of tile

        for y in 0..height {
            for x in 0..width {
                if affine {
                    // Get Screen Entry
                    let map_x = x / 8;
                    let map_y = y / 8;
                    let addr = map_start_addr + map_y * width / 8 + map_x;
                    let tile_num = get_bg(vram, addr) as usize;
                    
                    // Convert from tile to pixels
                    let (_, color_num) = Engine2D::get_color_from_tile(vram, get_bg,
                        tile_start_addr, tile_num, false, false, 8,
                        x % 8, y % 8, 0);
                    if color_num == 0 { continue }
                    pixels[y * width + x] = self.bg_palettes()[color_num] | 0x8000;
                } else {
                    // Get Screen Entry
                    let map_x = x / 8;
                    let map_y = y / 8;
                    let map_start_addr = map_start_addr + match bgcnt.screen_size {
                        0 => 0,
                        1 => if (map_x / 32) % 2 == 1 { 0x800 } else { 0 },
                        2 => if (map_y / 32) % 2 == 1 { 0x800 } else { 0 },
                        3 => {
                            let x_overflowed = (map_x / 32) % 2 == 1;
                            let y_overflowed = (map_y / 32) % 2 == 1;
                            if x_overflowed && y_overflowed { 0x800 * 3 }
                            else if y_overflowed { 0x800 * 2 }
                            else if x_overflowed { 0x800 * 1 }
                            else { 0 }
                        },
                        _ => unreachable!(),
                    };
                    let addr = map_start_addr + map_y * 32 * 2 + map_x * 2;
                    let screen_entry = u16::from_le_bytes([get_bg(vram, addr), get_bg(vram, addr + 1)]) as usize;
                    let tile_num = screen_entry & 0x3FF;
                    let flip_x = (screen_entry >> 10) & 0x1 != 0;
                    let flip_y = (screen_entry >> 11) & 0x1 != 0;
                    let palette_num = (screen_entry >> 12) & 0xF;
                    
                    // Convert from tile to pixels
                    let (palette_num, color_num) = Engine2D::get_color_from_tile(vram,
                        get_bg, tile_start_addr, tile_num, flip_x, flip_y, bit_depth,
                        x % 8, y % 8, palette_num);
                    if color_num == 0 { continue }
                    pixels[y * width + x] = self.bg_palettes()[palette_num * 16 + color_num] | 0x8000;
                }
            }
        }
        (pixels, width, height)
    }
}
