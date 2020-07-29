mod display;
mod debug;

use std::borrow::Cow;
use std::fs;

use nds_core::simplelog::*;
use nds_core::nds::{NDS, Engine, GraphicsType};

use display::Display;
use debug::*;
use imgui::*;

fn main() {
    std::env::set_current_dir("ROMs").unwrap();
    let instructions7_filter = LevelFilter::Off;
    let instructions9_filter = LevelFilter::Off;
    CombinedLogger::init(vec![
        TermLogger::new(LevelFilter::Warn, Config::default(), TerminalMode::Mixed),
        WriteLogger::new(instructions7_filter,
            ConfigBuilder::new()
            .set_time_level(LevelFilter::Off)
            .set_thread_level(LevelFilter::Off)
            .set_target_level(LevelFilter::Off)
            .set_location_level(LevelFilter::Off)
            .set_time_level(LevelFilter::Off)
            .set_max_level(LevelFilter::Off)
            .add_filter_allow_str("nds_core::arm7")
            .build(),
        fs::File::create("arm7.log").unwrap()),
        WriteLogger::new(instructions9_filter,
            ConfigBuilder::new()
            .set_time_level(LevelFilter::Off)
            .set_thread_level(LevelFilter::Off)
            .set_target_level(LevelFilter::Off)
            .set_location_level(LevelFilter::Off)
            .set_time_level(LevelFilter::Off)
            .set_max_level(LevelFilter::Off)
            .add_filter_allow_str("nds_core::arm9")
            .build(),
        fs::File::create("arm9.log").unwrap()),
    ]).unwrap();

    let mut imgui = Context::create();
    let mut display = Display::new(&mut imgui);
    
    let bios7 = fs::read("bios7.bin").unwrap();
    let bios9 = fs::read("bios9.bin").unwrap();
    let firmware = fs::read("firmware.bin").unwrap();
    let rom = fs::read("IRQ.nds").unwrap();
    let mut nds = NDS::new(bios7, bios9, firmware, rom);
    
    let engines = [Engine::A, Engine::B];
    let graphics_types = [GraphicsType::BG, GraphicsType::OBJ];
    let bgs = [im_str!("0"), im_str!("1"), im_str!("2"), im_str!("3")];

    let mut palettes_window = TextureWindow::new("Palettes");
    let mut palettes_engine = 0;
    let mut palettes_graphics_type = 0;

    let mut map_window = TextureWindow::new("Map");
    let mut map_engine = 0;
    let mut map_bg_i = 0;

    while !display.should_close() {
        nds.emulate_frame();
        let (palettes_pixels, palettes_width, palettes_height) =
            nds.render_palettes(engines[palettes_engine], graphics_types[palettes_graphics_type]);
        let (map_pixels, map_width, map_height) =
            nds.render_map(engines[map_engine], map_bg_i);
        display.render(&mut nds, &mut imgui,
            |ui, keys_pressed, _modifiers| {
            palettes_window.render(ui, &keys_pressed, palettes_pixels, palettes_width, palettes_height, || {
                let combo_width = ui.window_size()[0] * 0.3;

                ui.set_next_item_width(combo_width);
                ComboBox::new(im_str!("Engine"))
                .build_simple(ui, &mut palettes_engine,
                &engines, &(|i| Cow::from(ImString::new(i.label()))));

                ui.set_next_item_width(combo_width);
                ComboBox::new(im_str!("Graphics Type"))
                .build_simple(ui, &mut palettes_graphics_type,
                &graphics_types, &(|i| Cow::from(ImString::new(i.label()))));
            });

            map_window.render(ui, &keys_pressed, map_pixels, map_width, map_height, || {
                let combo_width = ui.window_size()[0] * 0.3;

                ui.set_next_item_width(combo_width);
                ComboBox::new(im_str!("Engine"))
                .build_simple(ui, &mut map_engine,
                &engines, &(|i| Cow::from(ImString::new(i.label()))));

                ui.set_next_item_width(combo_width);
                ComboBox::new(im_str!("BG"))
                .build_simple_string(ui, &mut map_bg_i, &bgs);
            });
        });
    }
}
