use super::registers::*;
use crate::hw::{mmu::IORegister, Scheduler};

pub struct Engine2D {
    // Registers
    dispcnt: DISPCNT,
    green_swap: bool,
    dispstat: DISPSTAT,
    vcount: u8,
    // Backgrounds
    bgcnts: [BGCNT; 4],
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
    bg_palettes: [u16; 0x100],
    obj_palettes: [u16; 0x100],
    // VRAM
    pub vram: Vec<u8>,
    pub oam: Vec<u8>,

    // Important Rendering Variables
    pixels: Vec<u16>,
    rendered_frame: bool,
    dot: u16,
    bg_lines: [[u16; Engine2D::WIDTH]; 4],
    objs_line: [OBJPixel; Engine2D::WIDTH],
    windows_lines: [[bool; Engine2D::WIDTH]; 3],
}

impl Engine2D {
    const TRANSPARENT_COLOR: u16 = 0x8000;
    const WIDTH: usize = 256;
    const HEIGHT: usize = 192;

    pub fn new() -> Self {
        Engine2D {
            // Registers
            dispcnt: DISPCNT::new(),
            green_swap: false,
            dispstat: DISPSTAT::new(),
            vcount: 0,
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
            // Color Special Effects
            bldcnt: BLDCNT::new(),
            bldalpha: BLDALPHA::new(),
            bldy: BLDY::new(),

            // Palettes
            bg_palettes: [0; 0x100],
            obj_palettes: [0; 0x100],
            // VRAM
            vram: vec![0; 0x18000],
            oam: vec![0; 0x400],

            // Important Rendering Variables
            pixels: vec![0; Engine2D::WIDTH * Engine2D::HEIGHT],
            rendered_frame: false,
            dot: 0,
            bg_lines: [[0; Engine2D::WIDTH]; 4],
            objs_line: [OBJPixel::none(); Engine2D::WIDTH],
            windows_lines: [[false; Engine2D::WIDTH]; 3],
        }
    }

    pub fn emulate_dot(&mut self) -> InterruptRequest {
        let mut interrupts = InterruptRequest::empty();
        if self.dot < 240 { // Visible
            self.dispstat.remove(DISPSTATFlags::HBLANK);
        } else { // HBlank
            if self.dot == 240 {
                if self.dispstat.contains(DISPSTATFlags::HBLANK_IRQ_ENABLE) {
                    interrupts.insert(InterruptRequest::HBLANK);
                }
            }
            if self.dot == 250 { // TODO: Take into account half
                self.dispstat.insert(DISPSTATFlags::HBLANK);
                //if self.vcount < 160 { self.hblank_called = true } // HDMA only occurs on visible scanlines
            }
        }
        if self.vcount < 160 && self.vcount != 227 { // Visible
            self.dispstat.remove(DISPSTATFlags::VBLANK);
            if self.dot == 241 { self.render_line() }
        } else { // VBlank
            if self.vcount == 160 && self.dot == 0 {
                //self.vblank_called = true;
                if self.dispstat.contains(DISPSTATFlags::VBLANK_IRQ_ENABLE) {
                    interrupts.insert(InterruptRequest::VBLANK)
                }
            }
            self.dispstat.insert(DISPSTATFlags::VBLANK);
        }

        if self.vcount == 160 && self.dot == 0 {
            self.rendered_frame = true;
        }

        self.dot += 1;
        if self.dot == 308 {
            self.dot = 0;
            if self.vcount == 227 {
                self.bgxs_latch = self.bgxs.clone();
                self.bgys_latch = self.bgys.clone();
            }
            self.vcount = (self.vcount + 1) % 228;
            if self.vcount == self.dispstat.vcount_setting {
                self.dispstat.insert(DISPSTATFlags::VCOUNTER);
                if self.dispstat.contains(DISPSTATFlags::VCOUNTER_IRQ_ENALBE) {
                    interrupts.insert(InterruptRequest::VCOUNTER_MATCH);
                }
            } else {
                self.dispstat.remove(DISPSTATFlags::VCOUNTER);
            }
        }
        interrupts
    }

    pub fn rendered_frame(&mut self) -> bool {
        let rendered_frame = self.rendered_frame;
        self.rendered_frame = false;
        rendered_frame
    }

    const OBJ_SIZES: [[(i16, u16); 3]; 4] = [
        [(8, 8), (16, 8), (8, 16)],
        [(16, 16), (32, 8), (8, 32)],
        [(32, 32), (32, 16), (16, 32)],
        [(64, 64), (64, 32), (32, 64)],
    ];

    fn render_line(&mut self) {
        if self.dispcnt.contains(DISPCNTFlags::DISPLAY_WINDOW0) { self.render_window(0) }
        if self.dispcnt.contains(DISPCNTFlags::DISPLAY_WINDOW1) { self.render_window(1) }
        if self.dispcnt.contains(DISPCNTFlags::DISPLAY_OBJ) { self.render_objs_line() }

        use BGMode::*;
        match self.dispcnt.mode {
            Mode0 => {
                let mut bgs: Vec<usize> = Vec::new();
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG0) { bgs.push(0) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG1) { bgs.push(1) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG2) { bgs.push(2) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG3) { bgs.push(3) }

                bgs.iter().for_each(|bg_i| self.render_text_line(*bg_i));
                self.process_lines(0, 3);
            },
            Mode1 => {
                let mut bgs: Vec<usize> = Vec::new();
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG0) { bgs.push(0) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG1) { bgs.push(1) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG2) { bgs.push(2) }

                bgs.iter().for_each(|bg_i| if *bg_i != 2 { self.render_text_line(*bg_i) }
                else { self.render_affine_line(*bg_i) });
                self.process_lines(0, 2);
            },
            Mode2 => {
                let mut bgs: Vec<usize> = Vec::new();
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG2) { bgs.push(2) }
                if self.dispcnt.contains(DISPCNTFlags::DISPLAY_BG3) { bgs.push(3) }

                bgs.iter().for_each(|bg_i| self.render_affine_line(*bg_i));
                self.process_lines(2, 3);
            },
            Mode3 => {
                let (mosaic_x, mosaic_y) = if self.bgcnts[2].mosaic {
                    (self.mosaic.bg_size.h_size as usize, self.mosaic.bg_size.v_size)
                } else { (1, 1) };
                for dot_x in 0..Engine2D::WIDTH {
                    let y = self.vcount / mosaic_y * mosaic_y;
                    let x = dot_x / mosaic_x * mosaic_x;
                    let addr = (y as usize * Engine2D::WIDTH + x) * 2;
                    self.bg_lines[2][dot_x] = u16::from_le_bytes([self.vram[addr], self.vram[addr + 1]]);
                }
                self.process_lines(2, 2);
            },
            Mode4 => {
                let (mosaic_x, mosaic_y) = if self.bgcnts[2].mosaic {
                    (self.mosaic.bg_size.h_size as usize, self.mosaic.bg_size.v_size)
                } else { (1, 1) };
                let y = self.vcount / mosaic_y * mosaic_y;
                let start_addr = if self.dispcnt.contains(DISPCNTFlags::DISPLAY_FRAME_SELECT) {
                    0xA000
                } else { 0 } + y as usize * Engine2D::WIDTH;
                for dot_x in 0..Engine2D::WIDTH {
                    let x = dot_x / mosaic_x * mosaic_x;
                    self.bg_lines[2][dot_x] = self.bg_palettes[self.vram[start_addr + x] as usize];
                }
                self.process_lines(2, 2);
            },
            Mode5 => {
                let (mosaic_x, mosaic_y) = if self.bgcnts[2].mosaic {
                    (self.mosaic.bg_size.h_size as usize, self.mosaic.bg_size.v_size as usize)
                } else { (1, 1) };
                let dot_y = self.vcount as usize;
                let y = dot_y / mosaic_y * mosaic_y;
                let start_addr = if self.dispcnt.contains(DISPCNTFlags::DISPLAY_FRAME_SELECT) {
                    0xA000usize
                } else { 0usize } + y * 160 * 2;
                for dot_x in 0..Engine2D::WIDTH {
                    self.bg_lines[2][dot_x] = if dot_x >= 160 || dot_y >= 128 {
                        self.bg_palettes[0]
                    } else {
                        let x = dot_x / mosaic_x * mosaic_x;
                        let addr = start_addr + 2 * x;
                        let pixel = u16::from_le_bytes([self.vram[addr], self.vram[addr + 1]]);
                        pixel
                    }
                }
                self.process_lines(2, 2);
            }
        }
    }
    
    fn process_lines(&mut self, start_line: usize, end_line: usize) {
        let start_index = self.vcount as usize * Engine2D::WIDTH;

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
        let mut pixels = &mut self.pixels;
        for dot_x in 0..Engine2D::WIDTH {
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
                if color != Engine2D::TRANSPARENT_COLOR && enabled[*bg_i] {
                    colors[i] = color;
                    layers[i] = Layer::from(*bg_i);
                    priorities[i] = *priority;
                    if i == 0 { i += 1 }
                    else { break }
                }
            }
            let obj_color = self.objs_line[dot_x].color;
            if enabled[4] && obj_color != Engine2D::TRANSPARENT_COLOR {
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
            pixels[start_index + dot_x] = final_color;
        }
    }

    fn render_window(&mut self, window_i: usize) {
        let y1 = self.winvs[window_i].coord1;
        let y2 = self.winvs[window_i].coord2;
        let y_in_window = if y1 > y2 {
            self.vcount < y1 && self.vcount >= y2
        } else {
            !(y1..y2).contains(&self.vcount)
        };
        if y_in_window {
            for dot_x in 0..Engine2D::WIDTH as u8 {
                self.windows_lines[window_i][dot_x as usize] = false;
            }
            return
        }
        
        let x1 = self.winhs[window_i].coord1;
        let x2 = self.winhs[window_i].coord2;
        if x1 > x2 {
            for dot_x in 0..Engine2D::WIDTH as u8 {
                self.windows_lines[window_i][dot_x as usize] = dot_x >= x1 || dot_x < x2;
            }
        } else {
            for dot_x in 0..Engine2D::WIDTH as u8 {
                self.windows_lines[window_i][dot_x as usize] = (x1..x2).contains(&dot_x);
            }
        }
    }

    fn render_objs_line(&mut self) {
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
            let (_, obj_height) = Engine2D::OBJ_SIZES[obj_size][obj_shape];
            let affine = obj[0] >> 8 & 0x1 != 0;
            let double_size_or_disable = obj[0] >> 9 & 0x1 != 0;
            if !affine && double_size_or_disable { return false }
            let obj_y_bounds = if double_size_or_disable { obj_height * 2 } else { obj_height };
            
            let obj_y = (obj[0] as u16) & 0xFF;
            let y_end = obj_y + obj_y_bounds;
            let y = self.vcount as u16 + if y_end > 256 { 256 } else { 0 };
            (obj_y..y_end).contains(&y)
        }).collect::<Vec<_>>();
        objs.sort_by_key(|a| (*a)[2] >> 10 & 0x3);
        let obj_window_enabled = self.dispcnt.flags.contains(DISPCNTFlags::DISPLAY_OBJ_WINDOW);

        for dot_x in 0..Engine2D::WIDTH {
            self.objs_line[dot_x] = OBJPixel::none();
            self.windows_lines[2][dot_x] = false;
            let mut set_color = false;
            for obj in objs.iter() {
                let obj_shape = (obj[0] >> 14 & 0x3) as usize;
                let obj_size = (obj[1] >> 14 & 0x3) as usize;
                let affine = obj[0] >> 8 & 0x1 != 0;
                let (obj_width, obj_height) = Engine2D::OBJ_SIZES[obj_size][obj_shape];
                let dot_x_signed = (dot_x as i16) / self.mosaic.obj_size.h_size as i16 * self.mosaic.obj_size.h_size as i16;
                let obj_x = (obj[1] & 0x1FF) as u16;
                let obj_x = if obj_x & 0x100 != 0 { 0xFE00 | obj_x } else { obj_x } as i16;
                let obj_y = (obj[0] & 0xFF) as u16;
                let double_size = obj[0] >> 9 & 0x1 != 0;
                let obj_x_bounds = if double_size { obj_width * 2 } else { obj_width };
                if !(obj_x..obj_x + obj_x_bounds).contains(&dot_x_signed) { continue }

                let base_tile_num = (obj[2] & 0x3FF) as usize;
                let x_diff = dot_x_signed - obj_x;
                let y = self.vcount / self.mosaic.obj_size.v_size * self.mosaic.obj_size.v_size;
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
                let bit_depth = if obj[0] >> 13 & 0x1 != 0 { 8 } else { 4 };
                let base_tile_num = if bit_depth == 8 { base_tile_num / 2 } else { base_tile_num };
                let tile_num = base_tile_num + if self.dispcnt.contains(DISPCNTFlags::OBJ_TILES1D) {
                    (y_diff as i16 / 8 * obj_width + x_diff) / 8
                } else { y_diff as i16 / 8 * 0x80 / (bit_depth as i16) + x_diff / 8 } as usize;
                let tile_x = x_diff % 8;
                let tile_y = y_diff % 8;
                let palette_num = (obj[2] >> 12 & 0xF) as usize;
                // Flipped at tile level, so no need to flip again
                let (palette_num, color_num) = self.get_color_from_tile(0x10000, tile_num,
                    false, false, bit_depth, tile_x as usize, tile_y as usize, palette_num);
                if color_num == 0 { continue }
                let mode = obj[0] >> 10 & 0x3;
                if mode == 2 {
                    self.windows_lines[2][dot_x] = obj_window_enabled;
                    if set_color { break } // Continue to look for color pixels
                } else if !set_color {
                    self.objs_line[dot_x] = OBJPixel {
                        color: self.obj_palettes[palette_num * 16 + color_num],
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
    
    fn render_affine_line(&mut self, bg_i: usize) {
        let mut base_x = self.bgxs_latch[bg_i - 2];
        let mut base_y = self.bgys_latch[bg_i - 2];
        self.bgxs_latch[bg_i - 2] += self.dmxs[bg_i - 2];
        self.bgys_latch[bg_i - 2] += self.dmys[bg_i - 2];
        let dx = self.dxs[bg_i - 2];
        let dy = self.dys[bg_i - 2];
        let bgcnt = self.bgcnts[bg_i];
        let tile_start_addr = bgcnt.tile_block as usize * 0x4000;
        let map_start_addr = bgcnt.map_block as usize * 0x800;
        let map_size = 128 << bgcnt.screen_size; // In Pixels
        let (mosaic_x, mosaic_y) = if bgcnt.mosaic {
            (self.mosaic.bg_size.h_size as usize, self.mosaic.bg_size.v_size as usize)
        } else { (1, 1) };

        for dot_x in 0..Engine2D::WIDTH {
            let (x_raw, y_raw) = (base_x.integer(), base_y.integer());
            base_x += dx;
            base_y += dy;
            let (x, y) = if x_raw < 0 || x_raw > map_size as i32 ||
            y_raw < 0 || y_raw > map_size as i32 {
                if bgcnt.wrap { ((x_raw % map_size as i32) as usize, (y_raw % map_size as i32) as usize) }
                else {
                    self.bg_lines[bg_i][dot_x] = Engine2D::TRANSPARENT_COLOR;
                    continue
                }
            } else { (x_raw as usize, y_raw as usize) };
            // Get Screen Entry
            let map_x = (x / mosaic_x * mosaic_x / 8) % (map_size / 8);
            let map_y = (y / mosaic_y * mosaic_y / 8) % (map_size / 8);
            let addr = map_start_addr + map_y * map_size / 8 + map_x;
            let tile_num = self.vram[addr] as usize;
            
            // Convert from tile to pixels
            let (_, color_num) = self.get_color_from_tile(tile_start_addr, tile_num,
                false, false, 8, x % 8, y % 8, 0);
            self.bg_lines[bg_i][dot_x] = if color_num == 0 { Engine2D::TRANSPARENT_COLOR }
            else { self.bg_palettes[color_num] };
        }
    }

    fn render_text_line(&mut self, bg_i: usize) {
        let x_offset = self.hofs[bg_i].offset as usize;
        let y_offset = self.vofs[bg_i].offset as usize;
        let bgcnt = self.bgcnts[bg_i];
        let tile_start_addr = bgcnt.tile_block as usize * 0x4000;
        let map_start_addr = bgcnt.map_block as usize * 0x800;
        let bit_depth = if bgcnt.bpp8 { 8 } else { 4 }; // Also bytes per row of tile
        let (mosaic_x, mosaic_y) = if bgcnt.mosaic {
            (self.mosaic.bg_size.h_size as usize, self.mosaic.bg_size.v_size as usize)
        } else { (1, 1) };

        let dot_y = self.vcount as usize;
        for dot_x in 0..Engine2D::WIDTH {
            let x = (dot_x + x_offset) / mosaic_x * mosaic_x;
            let y = (dot_y + y_offset) / mosaic_y * mosaic_y;
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
            let screen_entry = u16::from_le_bytes([self.vram[addr], self.vram[addr + 1]]) as usize;
            let tile_num = screen_entry & 0x3FF;
            let flip_x = (screen_entry >> 10) & 0x1 != 0;
            let flip_y = (screen_entry >> 11) & 0x1 != 0;
            let palette_num = (screen_entry >> 12) & 0xF;
            
            // Convert from tile to pixels
            let (palette_num, color_num) = self.get_color_from_tile(tile_start_addr, tile_num, flip_x, flip_y,
                bit_depth, x % 8, y % 8, palette_num);
            self.bg_lines[bg_i][dot_x] = if color_num == 0 { Engine2D::TRANSPARENT_COLOR }
            else { self.bg_palettes[palette_num * 16 + color_num]};
        }
    }

    fn get_color_from_tile(&self, tile_start_addr: usize, tile_num: usize, flip_x: bool, flip_y: bool,
        bit_depth: usize, tile_x: usize, tile_y: usize, palette_num: usize) -> (usize, usize) {
        let addr = tile_start_addr + 8 * bit_depth * tile_num;
        if tile_start_addr < 0x10000 && addr >= 0x10000 { return (0, 0) } // BG maps can't use OBJ tiles
        let tile_x = if flip_x { 7 - tile_x } else { tile_x };
        let tile_y = if flip_y { 7 - tile_y } else { tile_y };
        let tile = self.vram[addr + tile_y * bit_depth + tile_x / (8 / bit_depth)] as usize;
        if bit_depth == 8 {
            (0, tile)
        } else {
            (palette_num, ((tile >> 4 * (tile_x % 2)) & 0xF))
        }
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
            color: Engine2D::TRANSPARENT_COLOR,
            priority: 4,
            semitransparent: false,
        }
    }
}

impl Engine2D {
    pub fn read_register(&self, addr: u32) -> u8 {
        assert_eq!(addr >> 12, 0x04000);
        match addr & 0xFFF {
            0x000 => self.dispcnt.read(0),
            0x001 => self.dispcnt.read(1),
            0x002 => self.green_swap as u8,
            0x003 => 0, // Unused area of Green Swap
            0x004 => self.dispstat.read(0),
            0x005 => self.dispstat.read(1),
            0x006 => self.vcount as u8,
            0x007 => 0, // Unused area of VCOUNT
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
            0x050 => self.bldcnt.read(0),
            0x051 => self.bldcnt.read(1),
            0x052 => self.bldalpha.read(0),
            0x053 => self.bldalpha.read(1),
            0x054 => self.bldy.read(0),
            0x055 => self.bldy.read(1),
            _ => { warn!("Ignoring Engine2D Read at 0x{:08X}", addr); 0 },
        }
    }

    pub fn write_register(&mut self, scheduler: &mut Scheduler, addr: u32, value: u8) {
        assert_eq!(addr >> 12, 0x04000);
        match addr & 0xFFF {
            0x000 => self.dispcnt.write(scheduler, 0, value),
            0x001 => self.dispcnt.write(scheduler, 1, value),
            0x002 => self.green_swap = value & 0x1 != 0,
            0x003 => (),
            0x004 => self.dispstat.write(scheduler, 0, value),
            0x005 => self.dispstat.write(scheduler, 1, value),
            0x006 => (),
            0x007 => (),
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
            0x050 => self.bldcnt.write(scheduler, 0, value),
            0x051 => self.bldcnt.write(scheduler, 1, value),
            0x052 => self.bldalpha.write(scheduler, 0, value),
            0x053 => self.bldalpha.write(scheduler, 1, value),
            0x054 => self.bldy.write(scheduler, 0, value),
            0x055 => self.bldy.write(scheduler, 1, value),
            _ => { warn!("Ignoring Engine2D Write 0x{:08X} = {:02X}", addr, value) },
        }
    }

    pub fn read_palette_ram(&self, addr: u32) -> u8 {
        let addr = (addr & 0x3FF) as usize;
        let palettes = if addr < 0x200 { &self.bg_palettes } else { &self.obj_palettes };
        let index = (addr & 0x1FF) / 2;
        if addr % 2 == 0 {
            (palettes[index] >> 0) as u8
        } else {
            (palettes[index] >> 8) as u8
        }
    }

    pub fn write_palette_ram(&mut self, addr: u32, value: u8) {
        let addr = (addr & 0x3FF) as usize;
        let palettes = if addr < 0x200 { &mut self.bg_palettes } else { &mut self.obj_palettes };
        let index = (addr & 0x1FF) / 2;
        if addr % 2 == 0 {
            palettes[index] = palettes[index] & !0x00FF | (value as u16) << 0;
        } else {
            palettes[index] = palettes[index] & !0xFF00 | (value as u16) << 8 & !0x8000; // Clear high bit 
        }
    }

    pub fn parse_vram_addr(addr: u32) -> u32 {
        let addr = addr & 0x1_FFFF;
        if addr < 0x1_0000 { addr } else { addr & 0x1_7FFF }
    }

    pub fn parse_oam_addr(addr: u32) -> u32 {
        addr & 0x3FF
    }
}
