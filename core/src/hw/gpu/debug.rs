use super::{Engine2D, EngineType, GPU, VRAM, registers::BGMode};

impl GPU {
    pub fn render_palettes<F: Fn(usize) -> u16>(get_color: F, palettes_size: usize) -> (Vec<u16>, usize, usize) {
        let size = palettes_size * 8;
        let mut pixels = vec![0; size * size];
        for palette_y in 0..palettes_size {
            for palette_x in 0..palettes_size {
                let color_num = palette_y * palettes_size + palette_x;
                let start_i = (palette_y * size + palette_x) * 8;
                for y in 0..8 {
                    for x in 0..8 {
                        pixels[start_i + y * size + x] = get_color(color_num) | 0x8000;
                    }
                }
            }
        }
        (pixels, size, size)
    }

    pub fn render_tiles<E: EngineType, V, C>(vram: &VRAM, get_vram_byte: &V, get_color: &C, offset: usize, palette: usize,
        extended: bool, bpp8: bool) -> (Vec<u16>, usize, usize) where V: Fn(&VRAM, usize) -> u8, C: Fn(usize) -> u16 {
        let tile_start_addr = offset * 128 * 0x400;
        let bpp8 = bpp8 || extended;
        let (tiles_width, tiles_height) = if bpp8 { (64, 32) } else { (64, 64) };
        let (width, height) = (tiles_width * 8, tiles_height * 8);
        let mut pixels = vec![0; width * height];
        let bit_depth = if bpp8 { 8 } else { 4 };
        for tile_y in 0..tiles_height {
            for tile_x in 0..tiles_width {
                let start_i = (tile_y * width + tile_x) * 8;
                let tile_num = tile_y * tiles_width + tile_x;
                let addr = tile_start_addr + 8 * bit_depth * tile_num;
                for y in 0..8 {
                    for x in 0..8 {
                        let (palette_num, color_num) = Engine2D::<E>::get_color_from_tile(&vram,
                            get_vram_byte, addr, false, false, bit_depth,
                            x, y, palette);
                        if color_num == 0 { continue }
                        pixels[start_i + y * width + x] = 0x8000 | if extended {
                            get_color(palette * 16 + color_num)
                        } else {
                            get_color(palette_num * 16 + color_num)
                        };
                    }
                }
            }
        }
        (pixels, width, height)
    }
}

impl<E: EngineType> Engine2D<E> {
    pub fn render_map(&self, vram: &VRAM, bg_i: usize) -> (Vec<u16>, usize, usize) {
        let bgcnt = self.bgcnts[bg_i];
        // TODO: Implement Extended BGs
        let affine = match self.dispcnt.bg_mode {
            BGMode::Mode0 => false,
            BGMode::Mode1 => bg_i == 3,
            BGMode::Mode2 => bg_i >= 2,
            BGMode::Mode4 => bg_i == 2,
            _ => false,
        };
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
        let bit_depth = if bgcnt.bpp8 || affine { 8 } else { 4 }; // Also bytes per row of tile

        for y in 0..height {
            for x in 0..width {
                if affine {
                    // Get Screen Entry
                    let map_x = x / 8;
                    let map_y = y / 8;
                    let addr = map_start_addr + map_y * width / 8 + map_x;
                    let tile_num = vram.get_bg::<E, u8>(addr) as usize;
                    
                    // Convert from tile to pixels
                    let (_, color_num) = Engine2D::<E>::get_color_from_tile(vram, VRAM::get_bg::<E, u8>,
                        tile_start_addr + 8 * bit_depth * tile_num, false, false, bit_depth,
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
                    let screen_entry = vram.get_bg::<E, u16>(addr) as usize;
                    let tile_num = screen_entry & 0x3FF;
                    let flip_x = (screen_entry >> 10) & 0x1 != 0;
                    let flip_y = (screen_entry >> 11) & 0x1 != 0;
                    let palette_num = (screen_entry >> 12) & 0xF;
                    
                    // Convert from tile to pixels
                    let (palette_num, color_num) = Engine2D::<E>::get_color_from_tile(vram,
                        VRAM::get_bg::<E, u8>, tile_start_addr + 8 * bit_depth * tile_num,
                        flip_x, flip_y, bit_depth, x % 8, y % 8, palette_num);
                    if color_num == 0 { continue }
                    pixels[y * width + x] = self.bg_palettes()[palette_num * 16 + color_num] | 0x8000;
                }
            }
        }
        (pixels, width, height)
    }

    pub fn render_tiles(&self, vram: &VRAM, is_bg: bool, extended: bool, bpp8: bool, slot: usize, palette: usize,
    offset: usize) -> (Vec<u16>, usize, usize) {
        if is_bg {
            if extended {
                GPU::render_tiles::<E, _, _>(vram, &VRAM::get_bg::<E, u8>,
                &|i| vram.get_bg_ext_pal::<E>(slot, i),
                offset, palette, extended, bpp8)
            } else {
                GPU::render_tiles::<E, _, _>(vram, &VRAM::get_bg::<E, u8>,
                &|i| self.bg_palettes()[i], offset, palette, extended, bpp8)
            }
        } else {
            if extended {
                GPU::render_tiles::<E, _, _>(vram, &VRAM::get_obj::<E>,
                &|i| vram.get_obj_ext_pal::<E>(i),
                offset, palette, extended, bpp8)
            } else {
                GPU::render_tiles::<E, _, _>(vram, &VRAM::get_obj::<E>,
                &|i| self.obj_palettes()[i], offset, palette, extended, bpp8)
            }
        }
    }
}
