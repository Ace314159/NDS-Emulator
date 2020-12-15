use std::borrow::Cow;

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
    bank: u32,
}

impl DebugWindowState for VRAMWindowState {
    fn new() -> Self {
        VRAMWindowState {
            bank: 0,
        }
    }

    fn render(&mut self, ui: &Ui) {
        Slider::new(im_str!("Bank")).range(0 as u32..=8)
        .build(ui, &mut self.bank);
    }

    fn get_pixels(&self, nds: &mut NDS) -> (Vec<u16>, usize, usize) {
        nds.render_bank(self.bank as usize)
    }
}
