use std::borrow::Cow;
use std::collections::VecDeque;
use std::time::Instant;

use imgui::*;

use super::{DebugWindowState, Engine, GraphicsType, NDS};

pub struct PalettesWindowState {
    palettes_extended: bool,
    palettes_slot: u32,
    palettes_palette: u32,
    palettes_engine: usize,
    palettes_graphics_type: usize,
}

impl DebugWindowState for PalettesWindowState {
    fn new() -> Self {
        PalettesWindowState {
            palettes_extended: false,
            palettes_slot: 0,
            palettes_palette: 0,
            palettes_engine: 0,
            palettes_graphics_type: 0,
        }
    }

    fn render(&mut self, ui: &Ui) {
        let combo_width = ui.window_size()[0] * 0.3;

        ui.checkbox(im_str!("Extended"), &mut self.palettes_extended);
        if self.palettes_extended {
            if Self::GRAPHICS_TYPES[self.palettes_graphics_type] == GraphicsType::BG {
                Slider::new(im_str!("Slot")).range(0 as u32..=3)
                .build(ui, &mut self.palettes_slot);
            }
            Slider::new(im_str!("Palette")).range(0 as u32..=15)
            .build(ui, &mut self.palettes_palette);
        }

        ui.set_next_item_width(combo_width);
        ComboBox::new(im_str!("Engine"))
        .build_simple(ui, &mut self.palettes_engine,
        &Self::ENGINES, &(|i| Cow::from(ImString::new(i.label()))));

        ui.set_next_item_width(combo_width);
        ComboBox::new(im_str!("Graphics Type"))
        .build_simple(ui, &mut self.palettes_graphics_type,
        &Self::GRAPHICS_TYPES, &(|i| Cow::from(ImString::new(i.label()))));
    }

    fn get_pixels(&self, nds: &mut NDS) -> (Vec<u16>, usize, usize) {
        nds.render_palettes(self.palettes_extended, self.palettes_slot as usize, self.palettes_palette as usize,
            Self::ENGINES[self.palettes_engine], Self::GRAPHICS_TYPES[self.palettes_graphics_type])
    }
}

pub struct MapsWindowState {
    map_engine: usize,
    map_bg_i: u32,
}

impl DebugWindowState for MapsWindowState {
    fn new() -> Self {
        MapsWindowState {
            map_engine: 0,
            map_bg_i: 0,
        }
    }

    fn render(&mut self, ui: &Ui) {
        let combo_width = ui.window_size()[0] * 0.3;

        ui.set_next_item_width(combo_width);
        ComboBox::new(im_str!("Engine"))
        .build_simple(ui, &mut self.map_engine,
        &Self::ENGINES, &(|i| Cow::from(ImString::new(i.label()))));

        Slider::new(im_str!("BG")).range(0 as u32..=3)
        .build(ui, &mut self.map_bg_i);
    }

    fn get_pixels(&self, nds: &mut NDS) -> (Vec<u16>, usize, usize) {
        nds.render_map(Self::ENGINES[self.map_engine], self.map_bg_i as usize)
    }
}

pub struct TilesWindowState {
    tiles_engine: usize,
    tiles_graphics_type: usize,
    tiles_extended: bool,
    tiles_bitmap: bool,
    tiles_bpp8: bool,
    tiles_slot: u32,
    tiles_palette: u32,
    tiles_offset: u32,
}

impl TilesWindowState {
    const TILES_RANGES: [std::ops::RangeInclusive<u32>; 2] = [0 as u32..=3, 0 as u32..=1];
}

impl DebugWindowState for TilesWindowState {
    fn new() -> Self {
        TilesWindowState {
            tiles_engine: 0,
            tiles_graphics_type: 0,
            tiles_extended: false,
            tiles_bitmap: false,
            tiles_bpp8: false,
            tiles_slot: 0,
            tiles_palette: 0,
            tiles_offset: 0,
        }
    }

    fn render(&mut self, ui: &Ui) {
        let combo_width = ui.window_size()[0] * 0.3;

        ui.set_next_item_width(combo_width);
        ComboBox::new(im_str!("Engine"))
        .build_simple(ui, &mut self.tiles_engine,
        &Self::ENGINES, &(|i| Cow::from(ImString::new(i.label()))));

        ui.set_next_item_width(combo_width);
        ComboBox::new(im_str!("Graphics Type"))
        .build_simple(ui, &mut self.tiles_graphics_type,
        &Self::GRAPHICS_TYPES, &(|i| Cow::from(ImString::new(i.label()))));

        // TODO: Clean up UI - Dropdown with 4 options instead of checkboxes
        if !self.tiles_extended && !self.tiles_bpp8 {
            ui.checkbox(im_str!("Bitmap"), &mut self.tiles_bitmap);
        }
        
        if !self.tiles_bitmap {
            ui.checkbox(im_str!("Extended Palettes"), &mut self.tiles_extended);
            if !self.tiles_extended {
                ui.checkbox(im_str!("256 Colors"), &mut self.tiles_bpp8);
            } else if Self::GRAPHICS_TYPES[self.tiles_graphics_type] == GraphicsType::BG {
                Slider::new(im_str!("Palette Slot")).range(0 as u32..=3)
                .build(ui, &mut self.tiles_slot);
            }

            if self.tiles_extended || !self.tiles_bpp8 {
                Slider::new(im_str!("Palette")).range(0 as u32..=15)
                .build(ui, &mut self.tiles_palette);
            }

        }
        if Self::ENGINES[self.tiles_engine] == Engine::A {
            Slider::new(im_str!("Offset")).range(Self::TILES_RANGES[self.tiles_graphics_type].clone())
            .build(ui, &mut self.tiles_offset);
        }
    }

    fn get_pixels(&self, nds: &mut NDS) -> (Vec<u16>, usize, usize) {
        nds.render_tiles(Self::ENGINES[self.tiles_engine], Self::GRAPHICS_TYPES[self.tiles_graphics_type],
            self.tiles_extended, self.tiles_bitmap, self.tiles_bpp8, self.tiles_slot as usize,
            self.tiles_palette as usize, self.tiles_offset as usize)
    }
}

pub struct VRAMWindowState {
    ignore_alpha: bool,
    bank: u32,
}

impl DebugWindowState for VRAMWindowState {
    fn new() -> Self {
        VRAMWindowState {
            ignore_alpha: false,
            bank: 0,
        }
    }

    fn render(&mut self, ui: &Ui) {
        ui.checkbox(im_str!("Ignore alpha"), &mut self.ignore_alpha);

        Slider::new(im_str!("Bank")).range(0 as u32..=8)
        .build(ui, &mut self.bank);
    }

    fn get_pixels(&self, nds: &mut NDS) -> (Vec<u16>, usize, usize) {
        nds.render_bank(self.bank as usize, self.ignore_alpha)
    }
}

// TODO: Combine this and DebugWindow to avoid code duplication
pub struct StatsWindow {
    opened: bool,
    frame_times: VecDeque<f32>,
    frame_times_sum: f32,
    prev_frame_completed: Instant,
}

impl StatsWindow {
    pub const NUM_FRAME_TIMES: usize = 20 * 60;

    pub fn new() -> Self {
        StatsWindow {
            opened: false,
            frame_times: VecDeque::new(),
            frame_times_sum: 0.0,
            prev_frame_completed: Instant::now(),
        }
    }

    pub fn frame_completed(&mut self) {
        let cur_time = Instant::now();
        let frame_time = cur_time.duration_since(self.prev_frame_completed).as_secs_f32();
        self.prev_frame_completed = cur_time;
        if self.frame_times.len() == Self::NUM_FRAME_TIMES {
            self.frame_times_sum -= self.frame_times.pop_front().unwrap();
        }
        self.frame_times.push_back(frame_time);
        self.frame_times_sum += frame_time;
    }

    pub fn render(&mut self, ui: &Ui) {
        if !self.opened { return }
        let mut opened = self.opened;
        Window::new(im_str!("Performance Stats")) // TODO: Replace with const
        .opened(&mut opened)
        .build(ui, || {
            ui.plot_lines(im_str!("Frame Times"), self.frame_times.make_contiguous()).build();
            ui.text(format!("Average: {}", self.frame_times_sum / self.frame_times.len() as f32))
        });
        self.opened = opened;
    }

    pub fn menu_item(&mut self, ui: &Ui) {
        let clicked = MenuItem::new(im_str!("Performance Stats")).selected(self.opened).build(ui);
        if clicked { self.opened = !self.opened }
    }
}
