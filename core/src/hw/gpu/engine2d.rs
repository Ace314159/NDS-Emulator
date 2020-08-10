use super::registers::*;
use super::{EngineType, EngineA, GPU, VRAM};
use crate::hw::{mmu::IORegister, Scheduler};

pub struct Engine2D<E: EngineType> {
    // Registers
    pub(super) dispcnt: DISPCNT<E>,
    // Backgrounds
    pub(super) bgcnts: [BGCNT; 4],
    hofs: [OFS; 4],
    vofs: [OFS; 4],
    dxs: [RotationScalingParameter; 2],
    dmxs: [RotationScalingParameter; 2],
    dys: [RotationScalingParameter; 2],
    dmys: [RotationScalingParameter; 2],
    bgxs: [ReferencePointCoord; 2],
    bgys: [ReferencePointCoord; 2],
    bgxs_latch: [ReferencePointCoord; 2],
    bgys_latch: [ReferencePointCoord; 2],
    mosaic: MOSAIC,
    pub master_bright: MasterBright,
    // Windows
    winhs: [WindowDimensions; 2],
    winvs: [WindowDimensions; 2],
    win_0_cnt: WindowControl,
    win_1_cnt: WindowControl,
    win_out_cnt: WindowControl,
    win_obj_cnt: WindowControl,
    // Color Special Effects
    bldcnt: BLDCNT,
    bldalpha: BLDALPHA,
    bldy: BLDY,

    // Palettes
    bg_palettes: Vec<u16>,
    obj_palettes: Vec<u16>,
    pub oam: Vec<u8>,

    // Important Rendering Variables
    pixels: Vec<u16>,
    bg_lines: [[u16; GPU::WIDTH]; 4],
    objs_line: [OBJPixel; GPU::WIDTH],
    windows_lines: [[bool; GPU::WIDTH]; 3],
}

impl<E: EngineType> Engine2D<E> {
    const TRANSPARENT_COLOR: u16 = 0x8000;

    pub fn new() -> Self {
        Engine2D {
            // Registers
            dispcnt: DISPCNT::new(),
            // Backgrounds
            bgcnts: [BGCNT::new(); 4],
            hofs: [OFS::new(); 4],
            vofs: [OFS::new(); 4],
            dxs: [RotationScalingParameter::new(); 2],
            dmxs: [RotationScalingParameter::new(); 2],
            dys: [RotationScalingParameter::new(); 2],
            dmys: [RotationScalingParameter::new(); 2],
            bgxs: [ReferencePointCoord::new(); 2],
            bgys: [ReferencePointCoord::new(); 2],
            bgxs_latch: [ReferencePointCoord::new(); 2],
            bgys_latch: [ReferencePointCoord::new(); 2],
            winhs: [WindowDimensions::new(); 2],
            winvs: [WindowDimensions::new(); 2],
            win_0_cnt: WindowControl::new(),
            win_1_cnt: WindowControl::new(),
            win_out_cnt: WindowControl::new(),
            win_obj_cnt: WindowControl::new(),
            mosaic: MOSAIC::new(),
            master_bright: MasterBright::new(),
            // Color Special Effects
            bldcnt: BLDCNT::new(),
            bldalpha: BLDALPHA::new(),
            bldy: BLDY::new(),

            // Palettes
            bg_palettes: vec![0; GPU::PALETTE_SIZE],
            obj_palettes: vec![0; GPU::PALETTE_SIZE],
            // VRAM
            oam: vec![0; 0x400],

            // Important Rendering Variables
            pixels: vec![0; GPU::WIDTH * GPU::HEIGHT],
            bg_lines: [[0; GPU::WIDTH]; 4],
            objs_line: [OBJPixel::none(); GPU::WIDTH],
            windows_lines: [[false; GPU::WIDTH]; 3],
        }
    }

    const OBJ_SIZES: [[(i16, u16); 3]; 4] = [
        [(8, 8), (16, 8), (8, 16)],
        [(16, 16), (32, 8), (8, 32)],
        [(32, 32), (32, 16), (16, 32)],
        [(64, 64), (64, 32), (32, 64)],
    ];

    pub fn render_line(&mut self, vram: &VRAM, vcount: u16) {
        match self.dispcnt.display_mode {
            DisplayMode::Mode0 => for dot_x in 0..GPU::WIDTH {
                self.set_pixel(vcount, dot_x, 0x7FFF);
            },
            DisplayMode::Mode1 => self.render_normal_line(vram, vcount),
            DisplayMode::Mode2 => for dot_x in 0..GPU::WIDTH {
                let index = vcount as usize * GPU::WIDTH + dot_x;
                let color = if let Some(bank) = vram.get_lcdc_bank(self.dispcnt.vram_block) {
                    u16::from_le_bytes([bank[index * 2], bank[index * 2 + 1]])
                } else { 0 };
                self.set_pixel(vcount, dot_x, color);
            },
            DisplayMode::Mode3 => todo!(),
        }
    }

    fn render_normal_line(&mut self, vram: &VRAM, vcount: u16) {
        if self.dispcnt.contains(DISPCNTFlags::DISPLAY_WINDOW0) { self.render_window(vcount, 0) }
        if self.dispcnt.contains(DISPCNTFlags::DISPLAY_WINDOW1) { self.render_window(vcount, 1) }
        if self.dispcnt.contains(DISPCNTFlags::DISPLAY_OBJ) { self.render_objs_line(vram, vcount, ) }

        match self.dispcnt.bg_mode {
            BGMode::Mode0 => {
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG0) { self.render_text_line(vram, vcount, 0) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG1) { self.render_text_line(vram, vcount, 1) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG2) { self.render_text_line(vram, vcount, 2) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG3) { self.render_text_line(vram, vcount, 3) }
                self.process_lines(vcount, 0, 3);
            },
            BGMode::Mode1 => {
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG0) { self.render_text_line(vram, vcount, 0) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG1) { self.render_text_line(vram, vcount, 1) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG2) { self.render_text_line(vram, vcount, 2) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG3) { self.render_affine_line(vram, 2) }
                self.process_lines(vcount, 0, 3);
            },
            BGMode::Mode2 => {
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG0) { self.render_affine_line(vram, 3) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG1) { self.render_text_line(vram, vcount, 3) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG2) { self.render_affine_line(vram, 2) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG3) { self.render_affine_line(vram, 3) }
                self.process_lines(vcount, 0, 3);
            },
            BGMode::Mode3 => todo!(),
            BGMode::Mode4 => todo!(),
            BGMode::Mode5 => todo!(),
            BGMode::Mode6 => todo!(),
        }
    }
    
    fn process_lines(&mut self, vcount: u16, start_line: usize, end_line: usize) {
        let mut bgs : Vec<(usize, u8)> = Vec::new();
        for bg_i in start_line..=end_line {
            if self.dispcnt.bits() & (1 << (8 + bg_i)) != 0 { bgs.push((bg_i, self.bgcnts[bg_i].priority)) }
        }
        bgs.sort_by_key(|a| a.1);
        let master_enabled = [
            self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG0),
            self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG1),
            self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG2),
            self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG3),
            self.dispcnt.contains(DISPCNTFlags::DISPLAY_OBJ),
        ];
        for dot_x in 0..GPU::WIDTH {
            let window_control = if self.windows_lines[0][dot_x] {
                self.win_0_cnt
            } else if self.windows_lines[1][dot_x] {
                self.win_1_cnt
            } else if self.windows_lines[2][dot_x] {
                self.win_obj_cnt
            } else if self.dispcnt.windows_enabled() {
                self.win_out_cnt
            } else {
                WindowControl::all()
            };
            self.windows_lines[0][dot_x] = false;
            self.windows_lines[1][dot_x] = false;
            self.windows_lines[2][dot_x] = false;
            let enabled = [
                master_enabled[0] && window_control.bg0_enable,
                master_enabled[1] && window_control.bg1_enable,
                master_enabled[2] && window_control.bg2_enable,
                master_enabled[3] && window_control.bg3_enable,
                master_enabled[4] && window_control.obj_enable,
            ];

            // Store top 2 layers
            let mut colors = [self.bg_palettes[0], self.bg_palettes[0]]; // Default is backdrop color
            let mut layers = [Layer::BD, Layer::BD];
            let mut priorities = [4, 4];
            let mut i = 0;
            for (bg_i, priority) in bgs.iter() {
                let color = self.bg_lines[*bg_i][dot_x];
                if color != Engine2D::<E>::TRANSPARENT_COLOR && enabled[*bg_i] {
                    colors[i] = color;
                    layers[i] = Layer::from(*bg_i);
                    priorities[i] = *priority;
                    if i == 0 { i += 1 }
                    else { break }
                }
            }
            let obj_color = self.objs_line[dot_x].color;
            if enabled[4] && obj_color != Engine2D::<E>::TRANSPARENT_COLOR {
                if self.objs_line[dot_x].priority <= priorities[0] {
                    colors[1] = colors[0];
                    layers[1] = layers[0];
                    colors[0] = obj_color;
                    layers[0] = Layer::OBJ;
                    // Priority is irrelevant so no need to change it
                } else if self.objs_line[dot_x].priority <= priorities[1] {
                    colors[1] = obj_color;
                    layers[1] = Layer::OBJ;
                }
            }

            let trans_obj = layers[0] == Layer::OBJ && self.objs_line[dot_x].semitransparent;
            let target1_enabled = self.bldcnt.target_pixel1.enabled[layers[0] as usize] || trans_obj;
            let target2_enabled = self.bldcnt.target_pixel2.enabled[layers[1] as usize];
            let final_color = if window_control.color_special_enable && target1_enabled {
                let effect = if trans_obj && target2_enabled { ColorSFX::AlphaBlend } else { self.bldcnt.effect };
                match effect {
                    ColorSFX::None => colors[0],
                    ColorSFX::AlphaBlend => {
                        if target2_enabled {
                            let mut new_color = 0;
                            for i in (0..3).rev() {
                                let val1 = colors[0] >> (5 * i) & 0x1F;
                                let val2 = colors[1] >> (5 * i) & 0x1F;
                                let new_val = std::cmp::min(0x1F,
                                    (val1 * self.bldalpha.eva + val2 * self.bldalpha.evb) >> 4);
                                new_color = new_color << 5 | new_val;
                            }
                            new_color
                        } else { colors[0] }
                    },
                    ColorSFX::BrightnessInc => {
                        let mut new_color = 0;
                        for i in (0..3).rev() {
                            let val = colors[0] >> (5 * i) & 0x1F;
                            let new_val = val + (((0x1F - val) * self.bldy.evy as u16) >> 4);
                            new_color = new_color << 5 | new_val & 0x1F;
                        }
                        new_color
                    },
                    ColorSFX::BrightnessDec => {
                        let mut new_color = 0;
                        for i in (0..3).rev() {
                            let val = colors[0] >> (5 * i) & 0x1F;
                            let new_val = val - ((val * self.bldy.evy as u16) >> 4);
                            new_color = new_color << 5 | new_val & 0x1F;
                        }
                        new_color
                    },
                }
            } else { colors[0] };
            self.set_pixel(vcount, dot_x, final_color);
        }
    }

    fn render_window(&mut self, vcount: u16, window_i: usize) {
        let y1 = self.winvs[window_i].coord1;
        let y2 = self.winvs[window_i].coord2;
        let vcount = vcount as u8; // Only lower 8 bits compared
        let y_not_in_window = if y1 > y2 {
            vcount < y1 && vcount >= y2
        } else {
            !(y1..y2).contains(&vcount)
        };
        if y_not_in_window {
            for dot_x in 0..GPU::WIDTH {
                self.windows_lines[window_i][dot_x as usize] = false;
            }
            return
        }
        
        let x1 = self.winhs[window_i].coord1 as usize;
        let x2 = self.winhs[window_i].coord2 as usize;
        if x1 > x2 {
            for dot_x in 0..GPU::WIDTH {
                self.windows_lines[window_i][dot_x] = dot_x >= x1 || dot_x < x2;
            }
        } else {
            for dot_x in 0..GPU::WIDTH {
                self.windows_lines[window_i][dot_x] = (x1..x2).contains(&dot_x);
            }
        }
    }

    fn render_objs_line(&mut self, vram: &VRAM, vcount: u16) {
        let mut oam_parsed = [[0u16; 3]; 0x80];
        let mut affine_params = [[0u16; 4]; 0x20];
        self.oam.chunks(8).enumerate() // 1 OAM Entry, 1 Affine Parameter
        .for_each(|(i, chunk)| {
            oam_parsed[i][0] = u16::from_le_bytes([chunk[0], chunk[1]]);
            oam_parsed[i][1] = u16::from_le_bytes([chunk[2], chunk[3]]);
            oam_parsed[i][2] = u16::from_le_bytes([chunk[4], chunk[5]]);
            affine_params[i / 4][i % 4] = u16::from_le_bytes([chunk[6], chunk[7]]);
        });
        let mut objs = oam_parsed.iter().filter(|obj| {
            let obj_shape = (obj[0] >> 14 & 0x3) as usize;
            let obj_size = (obj[1] >> 14 & 0x3) as usize;
            let (_, obj_height) = Engine2D::<E>::OBJ_SIZES[obj_size][obj_shape];
            let affine = obj[0] >> 8 & 0x1 != 0;
            let double_size_or_disable = obj[0] >> 9 & 0x1 != 0;
            if !affine && double_size_or_disable { return false }
            let obj_y_bounds = if double_size_or_disable { obj_height * 2 } else { obj_height };
            
            let obj_y = (obj[0] as u16) & 0xFF;
            let y_end = obj_y + obj_y_bounds;
            let y = vcount + if y_end > 256 { 256 } else { 0 };
            (obj_y..y_end).contains(&y)
        }).collect::<Vec<_>>();
        objs.sort_by_key(|a| (*a)[2] >> 10 & 0x3);
        let obj_window_enabled = self.dispcnt.flags.contains(DISPCNTFlags::DISPLAY_OBJ_WINDOW);

        for dot_x in 0..GPU::WIDTH {
            self.objs_line[dot_x] = OBJPixel::none();
            self.windows_lines[2][dot_x] = false;
            let mut set_color = false;
            for obj in objs.iter() {
                let obj_shape = (obj[0] >> 14 & 0x3) as usize;
                let obj_size = (obj[1] >> 14 & 0x3) as usize;
                let affine = obj[0] >> 8 & 0x1 != 0;
                let (obj_width, obj_height) = Engine2D::<E>::OBJ_SIZES[obj_size][obj_shape];
                let dot_x_signed = (dot_x as i16) / self.mosaic.obj_size.h_size as i16 * self.mosaic.obj_size.h_size as i16;
                let obj_x = (obj[1] & 0x1FF) as u16;
                let obj_x = if obj_x & 0x100 != 0 { 0xFE00 | obj_x } else { obj_x } as i16;
                let obj_y = (obj[0] & 0xFF) as u16;
                let double_size = obj[0] >> 9 & 0x1 != 0;
                let obj_x_bounds = if double_size { obj_width * 2 } else { obj_width };
                if !(obj_x..obj_x + obj_x_bounds).contains(&dot_x_signed) { continue }

                let base_tile_num = (obj[2] & 0x3FF) as usize;
                let x_diff = dot_x_signed - obj_x;
                let y = vcount / self.mosaic.obj_size.v_size * self.mosaic.obj_size.v_size;
                let y_diff = (y as u16).wrapping_sub(obj_y) & 0xFF;
                let (x_diff, y_diff) = if affine {
                    let (x_diff, y_diff) = if double_size {
                        (x_diff - obj_width / 2, y_diff as i16 - obj_height as i16 / 2)
                    } else { (x_diff, y_diff as i16) };
                    let aff_param = obj[1] >> 9 & 0x1F;
                    let params = affine_params[aff_param as usize];
                    let (pa, pb, pc, pd) = (
                        RotationScalingParameter::get_float_from_u16(params[0]),
                        RotationScalingParameter::get_float_from_u16(params[1]),
                        RotationScalingParameter::get_float_from_u16(params[2]),
                        RotationScalingParameter::get_float_from_u16(params[3]),
                    );
                    let (x_offset, y_offset) = (obj_width as f64 / 2.0, obj_height as f64 / 2.0);
                    let (x_raw, y_raw) = (
                        pa * (x_diff as f64 - x_offset) + pb * (y_diff as f64 - y_offset) + x_offset,
                        pc * (x_diff as f64 - x_offset) + pd * (y_diff as f64 - y_offset) + y_offset,
                    );
                    if x_raw < 0.0 || y_raw < 0.0 || x_raw >= obj_width as f64 || y_raw >= obj_height as f64 { continue }
                    (x_raw as u16 as i16, y_raw as u16)
                } else {
                    let flip_x = obj[1] >> 12 & 0x1 != 0;
                    let flip_y = obj[1] >> 13 & 0x1 != 0;
                    (
                        if flip_x { obj_width - 1 - x_diff } else { x_diff },
                        if flip_y { obj_height - 1 - y_diff } else { y_diff },
                    )
                };
                let bpp8 = obj[0] >> 13 & 0x1 != 0;
                let bit_depth = if bpp8 { 8 } else { 4 };
                let (boundary, tile_offset) = if self.dispcnt.contains(DISPCNTFlags::TILE_OBJ_1D) {
                    (32 << self.dispcnt.tile_obj_1d_bound, (y_diff as i16 / 8 * obj_width + x_diff) / 8)
                } else { (32, y_diff as i16 / 8 * 0x80 / (bit_depth as i16) + x_diff / 8) };
                let addr = boundary * base_tile_num + tile_offset as usize * bit_depth * 8;
                let tile_x = x_diff % 8;
                let tile_y = y_diff % 8;
                let original_palette_num = (obj[2] >> 12 & 0xF) as usize;
                // Flipped at tile level, so no need to flip again
                let (palette_num, color_num) = Engine2D::<E>::get_color_from_tile(vram,
                    VRAM::get_obj::<E>, addr, false, false,
                    bit_depth, tile_x as usize, tile_y as usize, original_palette_num);
                if color_num == 0 { continue }
                let mode = obj[0] >> 10 & 0x3;
                if mode == 2 {
                    self.windows_lines[2][dot_x] = obj_window_enabled;
                    if set_color { break } // Continue to look for color pixels
                } else if !set_color {
                    let color = if bpp8 && self.dispcnt.contains(DISPCNTFlags::OBJ_EXTENDED_PALETTES) {
                        vram.get_obj_ext_pal::<E>(original_palette_num * 16 + color_num)
                    } else {
                        self.obj_palettes[palette_num * 16 + color_num]
                    };
                    self.objs_line[dot_x] = OBJPixel {
                        color,
                        priority: (obj[2] >> 10 & 0x3) as u8,
                        semitransparent: mode == 1,
                    };
                    set_color = true;
                    // Continue to look for OBJ window pixels if not yet found and window is enabled
                    if self.windows_lines[2][dot_x] || !obj_window_enabled { break }
                }
            }
        }
    }
    
    fn render_affine_line(&mut self, vram: &VRAM, bg_i: usize) {
        let mut base_x = self.bgxs_latch[bg_i - 2];
        let mut base_y = self.bgys_latch[bg_i - 2];
        self.bgxs_latch[bg_i - 2] += self.dmxs[bg_i - 2];
        self.bgys_latch[bg_i - 2] += self.dmys[bg_i - 2];
        let dx = self.dxs[bg_i - 2];
        let dy = self.dys[bg_i - 2];
        let bgcnt = self.bgcnts[bg_i];
        let tile_start_addr = self.calc_tile_start_addr(&bgcnt);
        let map_start_addr = self.calc_map_start_addr(&bgcnt);
        let bit_depth = if bgcnt.bpp8 { 8 } else { 4 }; // Also bytes per row of tile
        let map_size = 128 << bgcnt.screen_size; // In Pixels
        let (mosaic_x, mosaic_y) = if bgcnt.mosaic {
            (self.mosaic.bg_size.h_size as usize, self.mosaic.bg_size.v_size as usize)
        } else { (1, 1) };

        for dot_x in 0..GPU::WIDTH {
            let (x_raw, y_raw) = (base_x.integer(), base_y.integer());
            base_x += dx;
            base_y += dy;
            let (x, y) = if x_raw < 0 || x_raw > map_size as i32 ||
            y_raw < 0 || y_raw > map_size as i32 {
                if bgcnt.wrap { ((x_raw % map_size as i32) as usize, (y_raw % map_size as i32) as usize) }
                else {
                    self.bg_lines[bg_i][dot_x] = Engine2D::<E>::TRANSPARENT_COLOR;
                    continue
                }
            } else { (x_raw as usize, y_raw as usize) };
            // Get Screen Entry
            let map_x = (x / mosaic_x * mosaic_x / 8) % (map_size / 8);
            let map_y = (y / mosaic_y * mosaic_y / 8) % (map_size / 8);
            let addr = map_start_addr + map_y * map_size / 8 + map_x;
            let tile_num = vram.get_bg::<E>(addr) as usize;
            
            // Convert from tile to pixels
            let (_, color_num) = Engine2D::<E>::get_color_from_tile(vram, VRAM::get_bg::<E>,
                tile_start_addr + 8 * bit_depth + tile_num, false, false, bit_depth,
                x % 8, y % 8, 0);
            self.bg_lines[bg_i][dot_x] = if color_num == 0 { Engine2D::<E>::TRANSPARENT_COLOR }
            else { self.bg_palettes[color_num] };
        }
    }

    fn render_text_line(&mut self, vram: &VRAM, vcount: u16, bg_i: usize) {
        let x_offset = self.hofs[bg_i].offset as usize;
        let y_offset = self.vofs[bg_i].offset as usize;
        let bgcnt = self.bgcnts[bg_i];
        let tile_start_addr = self.calc_tile_start_addr(&bgcnt);
        let map_start_addr = self.calc_map_start_addr(&bgcnt);
        let bit_depth = if bgcnt.bpp8 { 8 } else { 4 }; // Also bytes per row of tile
        /*let (mosaic_x, mosaic_y) = if bgcnt.mosaic {
            (self.mosaic.bg_size.h_size as usize, self.mosaic.bg_size.v_size as usize)
        } else { (1, 1) };*/

        let dot_y = vcount as usize;
        for dot_x in 0..GPU::WIDTH {
            let x = dot_x + x_offset;// / mosaic_x * mosaic_x;
            let y = dot_y + y_offset;// / mosaic_y * mosaic_y;
            // Get Screen Entry
            let mut map_x = x / 8;
            let mut map_y = y / 8;
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
            map_x %= 32;
            map_y %= 32;
            let addr = map_start_addr + map_y * 32 * 2 + map_x * 2;
            let screen_entry = u16::from_le_bytes([vram.get_bg::<E>(addr), vram.get_bg::<E>(addr + 1)]) as usize;
            let tile_num = screen_entry & 0x3FF;
            let flip_x = (screen_entry >> 10) & 0x1 != 0;
            let flip_y = (screen_entry >> 11) & 0x1 != 0;
            let palette_num = (screen_entry >> 12) & 0xF;
            
            // Convert from tile to pixels
            let (palette_num, color_num) = Engine2D::<E>::get_color_from_tile(vram,
                VRAM::get_bg::<E>, tile_start_addr + 8 * bit_depth * tile_num, flip_x, flip_y, bit_depth,
                x % 8, y % 8, palette_num);
            self.bg_lines[bg_i][dot_x] = if color_num == 0 { Engine2D::<E>::TRANSPARENT_COLOR }
            else if bgcnt.bpp8 & self.dispcnt.contains(DISPCNTFlags::BG_EXTENDED_PALETTES) {
                // Wrap bit is Change Ext Palette Slot for BG0/BG1
                let slot = if bg_i < 2 && bgcnt.wrap { bg_i + 2 } else { bg_i };
                vram.get_bg_ext_pal::<E>(slot, color_num)
            } else { self.bg_palettes[palette_num * 16 + color_num] };
        }
    }

    pub(super) fn get_color_from_tile<F: Fn(&VRAM, usize) -> u8>(vram: &VRAM, get_vram_byte: F, addr: usize,
        flip_x: bool, flip_y: bool, bit_depth: usize, tile_x: usize, tile_y: usize, palette_num: usize) -> (usize, usize) {
        let tile_x = if flip_x { 7 - tile_x } else { tile_x };
        let tile_y = if flip_y { 7 - tile_y } else { tile_y };
        let tile = get_vram_byte(vram, addr + tile_y * bit_depth + tile_x / (8 / bit_depth)) as usize;
        if bit_depth == 8 {
            (0, tile)
        } else {
            (palette_num, ((tile >> 4 * (tile_x % 2)) & 0xF))
        }
    }

    pub fn latch_affine(&mut self) {
        self.bgxs_latch = self.bgxs.clone();
        self.bgys_latch = self.bgys.clone();
    }

    pub(super) fn calc_tile_start_addr(&self, bgcnt: &BGCNT) -> usize {
        self.dispcnt.char_base as usize * 0x1_0000 + bgcnt.tile_block as usize * 0x4000
    }

    pub(super) fn calc_map_start_addr(&self, bgcnt: &BGCNT) -> usize {
        self.dispcnt.screen_base as usize * 0x1_0000 + bgcnt.map_block as usize * 0x800
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Layer {
    BG0 = 0,
    BG1 = 1,
    BG2 = 2,
    BG3 = 3,
    OBJ = 4,
    BD = 5,
}

impl Layer {
    pub fn from(value: usize) -> Layer {
        use Layer::*;
        match value {
            0 => BG0,
            1 => BG1,
            2 => BG2,
            3 => BG3,
            4 => OBJ,
            5 => BD,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy)]
struct OBJPixel {
    color: u16,
    priority: u8,
    semitransparent: bool,
}

impl OBJPixel {
    pub fn none() -> OBJPixel {
        OBJPixel {
            color: Engine2D::<EngineA>::TRANSPARENT_COLOR,
            priority: 4,
            semitransparent: false,
        }
    }
}

impl<E: EngineType> Engine2D<E>{
    pub fn read_register(&self, addr: u32) -> u8 {
        assert_eq!((addr >> 12) & !0x1, 0x04000);
        match addr & 0xFFF {
            0x000 => self.dispcnt.read(0),
            0x001 => self.dispcnt.read(1),
            0x002 => self.dispcnt.read(2),
            0x003 => self.dispcnt.read(3),
            // DISPSTAT and VCOUNT are read in GPU
            0x008 => self.bgcnts[0].read(0),
            0x009 => self.bgcnts[0].read(1),
            0x00A => self.bgcnts[1].read(0),
            0x00B => self.bgcnts[1].read(1),
            0x00C => self.bgcnts[2].read(0),
            0x00D => self.bgcnts[2].read(1),
            0x00E => self.bgcnts[3].read(0),
            0x00F => self.bgcnts[3].read(1),
            0x010 => self.hofs[0].read(0),
            0x011 => self.hofs[0].read(1),
            0x012 => self.vofs[0].read(0),
            0x013 => self.vofs[0].read(1),
            0x014 => self.hofs[1].read(0),
            0x015 => self.hofs[1].read(1),
            0x016 => self.vofs[1].read(0),
            0x017 => self.vofs[1].read(1),
            0x018 => self.hofs[2].read(0),
            0x019 => self.hofs[2].read(1),
            0x01A => self.vofs[2].read(0),
            0x01B => self.vofs[2].read(1),
            0x01C => self.hofs[3].read(0),
            0x01D => self.hofs[3].read(1),
            0x01E => self.vofs[3].read(0),
            0x01F => self.vofs[3].read(1),
            0x020 => self.dxs[0].read(0),
            0x021 => self.dxs[0].read(1),
            0x022 => self.dmxs[0].read(0),
            0x023 => self.dmxs[0].read(1),
            0x024 => self.dys[0].read(0),
            0x025 => self.dys[0].read(1),
            0x026 => self.dmys[0].read(0),
            0x027 => self.dmys[0].read(1),
            0x028 => self.bgxs[0].read(0),
            0x029 => self.bgxs[0].read(1),
            0x02A => self.bgxs[0].read(2),
            0x02B => self.bgxs[0].read(3),
            0x02C => self.bgys[0].read(0),
            0x02D => self.bgys[0].read(1),
            0x02E => self.bgys[0].read(2),
            0x02F => self.bgys[0].read(3),
            0x030 => self.dxs[1].read(0),
            0x031 => self.dxs[1].read(1),
            0x032 => self.dmxs[1].read(0),
            0x033 => self.dmxs[1].read(1),
            0x034 => self.dys[1].read(0),
            0x035 => self.dys[1].read(1),
            0x036 => self.dmys[1].read(0),
            0x037 => self.dmys[1].read(1),
            0x038 => self.bgxs[1].read(0),
            0x039 => self.bgxs[1].read(1),
            0x03A => self.bgxs[1].read(2),
            0x03B => self.bgxs[1].read(3),
            0x03C => self.bgys[1].read(0),
            0x03D => self.bgys[1].read(1),
            0x03E => self.bgys[1].read(2),
            0x03F => self.bgys[1].read(3),
            0x040 => self.winhs[0].read(0),
            0x041 => self.winhs[0].read(1),
            0x042 => self.winhs[1].read(0),
            0x043 => self.winhs[1].read(1),
            0x044 => self.winvs[0].read(0),
            0x045 => self.winvs[0].read(1),
            0x046 => self.winvs[1].read(0),
            0x047 => self.winvs[1].read(1),
            0x048 => self.win_0_cnt.read(0),
            0x049 => self.win_1_cnt.read(0),
            0x04A => self.win_out_cnt.read(0),
            0x04B => self.win_obj_cnt.read(0),
            0x04C => self.mosaic.read(0),
            0x04D => self.mosaic.read(1),
            0x04E ..= 0x04F => 0,
            0x050 => self.bldcnt.read(0),
            0x051 => self.bldcnt.read(1),
            0x052 => self.bldalpha.read(0),
            0x053 => self.bldalpha.read(1),
            0x054 => self.bldy.read(0),
            0x055 => self.bldy.read(1),
            0x056 ..= 0x05F => 0,
            _ => { warn!("Ignoring Engine2D Read at 0x{:08X}", addr); 0 },
        }
    }

    pub fn write_register(&mut self, scheduler: &mut Scheduler, addr: u32, value: u8) {
        assert_eq!((addr >> 12) & !0x1, 0x04000);
        match addr & 0xFFF {
            0x000 => self.dispcnt.write(scheduler, 0, value),
            0x001 => self.dispcnt.write(scheduler, 1, value),
            0x002 => self.dispcnt.write(scheduler, 2, value),
            0x003 => self.dispcnt.write(scheduler, 3, value),
            // DISPSTAT and VCOUNT are written in GPU
            0x008 => self.bgcnts[0].write(scheduler, 0, value),
            0x009 => self.bgcnts[0].write(scheduler, 1, value),
            0x00A => self.bgcnts[1].write(scheduler, 0, value),
            0x00B => self.bgcnts[1].write(scheduler, 1, value),
            0x00C => self.bgcnts[2].write(scheduler, 0, value),
            0x00D => self.bgcnts[2].write(scheduler, 1, value),
            0x00E => self.bgcnts[3].write(scheduler, 0, value),
            0x00F => self.bgcnts[3].write(scheduler, 1, value),
            0x010 => self.hofs[0].write(scheduler, 0, value),
            0x011 => self.hofs[0].write(scheduler, 1, value),
            0x012 => self.vofs[0].write(scheduler, 0, value),
            0x013 => self.vofs[0].write(scheduler, 1, value),
            0x014 => self.hofs[1].write(scheduler, 0, value),
            0x015 => self.hofs[1].write(scheduler, 1, value),
            0x016 => self.vofs[1].write(scheduler, 0, value),
            0x017 => self.vofs[1].write(scheduler, 1, value),
            0x018 => self.hofs[2].write(scheduler, 0, value),
            0x019 => self.hofs[2].write(scheduler, 1, value),
            0x01A => self.vofs[2].write(scheduler, 0, value),
            0x01B => self.vofs[2].write(scheduler, 1, value),
            0x01C => self.hofs[3].write(scheduler, 0, value),
            0x01D => self.hofs[3].write(scheduler, 1, value),
            0x01E => self.vofs[3].write(scheduler, 0, value),
            0x01F => self.vofs[3].write(scheduler, 1, value),
            0x020 => self.dxs[0].write(scheduler, 0, value),
            0x021 => self.dxs[0].write(scheduler, 1, value),
            0x022 => self.dmxs[0].write(scheduler, 0, value),
            0x023 => self.dmxs[0].write(scheduler, 1, value),
            0x024 => self.dys[0].write(scheduler, 0, value),
            0x025 => self.dys[0].write(scheduler, 1, value),
            0x026 => self.dmys[0].write(scheduler, 0, value),
            0x027 => self.dmys[0].write(scheduler, 1, value),
            0x028 => { self.bgxs[0].write(scheduler, 0, value); self.bgxs_latch[0] = self.bgxs[0].clone() },
            0x029 => { self.bgxs[0].write(scheduler, 1, value); self.bgxs_latch[0] = self.bgxs[0].clone() },
            0x02A => { self.bgxs[0].write(scheduler, 2, value); self.bgxs_latch[0] = self.bgxs[0].clone() },
            0x02B => { self.bgxs[0].write(scheduler, 3, value); self.bgxs_latch[0] = self.bgxs[0].clone() },
            0x02C => { self.bgys[0].write(scheduler, 0, value); self.bgys_latch[0] = self.bgys[0].clone() },
            0x02D => { self.bgys[0].write(scheduler, 1, value); self.bgys_latch[0] = self.bgys[0].clone() },
            0x02E => { self.bgys[0].write(scheduler, 2, value); self.bgys_latch[0] = self.bgys[0].clone() },
            0x02F => { self.bgys[0].write(scheduler, 3, value); self.bgys_latch[0] = self.bgys[0].clone() },
            0x030 => self.dxs[1].write(scheduler, 0, value),
            0x031 => self.dxs[1].write(scheduler, 1, value),
            0x032 => self.dmxs[1].write(scheduler, 0, value),
            0x033 => self.dmxs[1].write(scheduler, 1, value),
            0x034 => self.dys[1].write(scheduler, 0, value),
            0x035 => self.dys[1].write(scheduler, 1, value),
            0x036 => self.dmys[1].write(scheduler, 0, value),
            0x037 => self.dmys[1].write(scheduler, 1, value),
            0x038 => self.bgxs[1].write(scheduler, 0, value),
            0x039 => self.bgxs[1].write(scheduler, 1, value),
            0x03A => self.bgxs[1].write(scheduler, 2, value),
            0x03B => self.bgxs[1].write(scheduler, 3, value),
            0x03C => self.bgys[1].write(scheduler, 0, value),
            0x03D => self.bgys[1].write(scheduler, 1, value),
            0x03E => self.bgys[1].write(scheduler, 2, value),
            0x03F => self.bgys[1].write(scheduler, 3, value),
            0x040 => self.winhs[0].write(scheduler, 0, value),
            0x041 => self.winhs[0].write(scheduler, 1, value),
            0x042 => self.winhs[1].write(scheduler, 0, value),
            0x043 => self.winhs[1].write(scheduler, 1, value),
            0x044 => self.winvs[0].write(scheduler, 0, value),
            0x045 => self.winvs[0].write(scheduler, 1, value),
            0x046 => self.winvs[1].write(scheduler, 0, value),
            0x047 => self.winvs[1].write(scheduler, 1, value),
            0x048 => self.win_0_cnt.write(scheduler, 0, value),
            0x049 => self.win_1_cnt.write(scheduler, 0, value),
            0x04A => self.win_out_cnt.write(scheduler, 0, value),
            0x04B => self.win_obj_cnt.write(scheduler, 0, value),
            0x04C => self.mosaic.write(scheduler, 0, value),
            0x04D => self.mosaic.write(scheduler, 1, value),
            0x04E ..= 0x04F => (),
            0x050 => self.bldcnt.write(scheduler, 0, value),
            0x051 => self.bldcnt.write(scheduler, 1, value),
            0x052 => self.bldalpha.write(scheduler, 0, value),
            0x053 => self.bldalpha.write(scheduler, 1, value),
            0x054 => self.bldy.write(scheduler, 0, value),
            0x055 => self.bldy.write(scheduler, 1, value),
            0x056 ..= 0x05F => (),
            _ => warn!("Ignoring Engine2D Write 0x{:08X} = {:02X}", addr, value),
        }
    }

    pub fn read_palette_ram(&self, addr: u32) -> u8 {
        let addr = addr as usize & (2 * GPU::PALETTE_SIZE - 1);
        let palettes = if addr < GPU::PALETTE_SIZE { &self.bg_palettes } else { &self.obj_palettes };
        let index = (addr & GPU::PALETTE_SIZE - 1) / 2;
        if addr % 2 == 0 {
            (palettes[index] >> 0) as u8
        } else {
            warn!("Reading Palette - 15th bit could be wrong: 0x{:X}", palettes[index] >> 8);
            (palettes[index] >> 8) as u8
        }
    }

    fn set_pixel(&mut self, vcount: u16, dot_x: usize, color: u16) {
        self.pixels[vcount as usize * GPU::WIDTH + dot_x] = self.master_bright.apply(color);
    }

    pub fn write_palette_ram(&mut self, addr: usize, value: u16) {
        let addr = addr as usize & (2 * GPU::PALETTE_SIZE - 1);
        let palettes = if addr < GPU::PALETTE_SIZE { &mut self.bg_palettes } else { &mut self.obj_palettes };
        let index = (addr & GPU::PALETTE_SIZE - 1) / 2;
        palettes[index] = value;
        // if value & 0x8000 != 0 { warn!("Writing to palette with 15th bit set - Reading could be inaccurate: 0x{:X} ", value) }
    }

    pub fn bg_palettes(&self) -> &Vec<u16> { &self.bg_palettes }
    pub fn obj_palettes(&self) -> &Vec<u16> { &self.obj_palettes }
    pub fn pixels(&self) -> &Vec<u16> { &self.pixels }
}
