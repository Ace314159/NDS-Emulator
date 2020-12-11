mod display;
mod debug;

use std::fs;
use std::path::PathBuf;

use nds_core::simplelog::*;
use nds_core::nds::{NDS, Engine, GraphicsType};

use display::Display;
use debug::*;
use imgui::*;

fn main() {
    let rom_file = "examples/3D/Simple_Tri.nds";

    std::env::set_current_dir("ROMs").unwrap();
    let arm7_file_name = "arm7.log";
    let arm9_file_name = "arm9.log";
    let instructions7_filter = LevelFilter::Off;
    let instructions9_filter = LevelFilter::Off;

    let arm7_file = fs::File::create(arm7_file_name);
    let arm9_file = fs::File::create(arm9_file_name);
    let mut loggers: Vec<Box<dyn SharedLogger>> = vec![
        TermLogger::new(LevelFilter::Warn, Config::default(), TerminalMode::Mixed),
    ];
    if let Ok(file) = arm7_file {
        loggers.push(WriteLogger::new(instructions7_filter,
            ConfigBuilder::new()
            .set_time_level(LevelFilter::Off)
            .set_thread_level(LevelFilter::Off)
            .set_target_level(LevelFilter::Off)
            .set_location_level(LevelFilter::Off)
            .set_time_level(LevelFilter::Off)
            .set_max_level(LevelFilter::Off)
            .add_filter_allow_str("nds_core::arm7")
            .build(),
        file));
    }
    if let Ok(file) = arm9_file {
        loggers.push(WriteLogger::new(instructions9_filter,
            ConfigBuilder::new()
            .set_time_level(LevelFilter::Off)
            .set_thread_level(LevelFilter::Off)
            .set_target_level(LevelFilter::Off)
            .set_location_level(LevelFilter::Off)
            .set_time_level(LevelFilter::Off)
            .set_max_level(LevelFilter::Off)
            .add_filter_allow_str("nds_core::arm9")
            .build(),
        file))
    }
    CombinedLogger::init(loggers).unwrap();

    let mut imgui = Context::create();
    let mut display = Display::new(&mut imgui);
    
    let bios7 = fs::read("bios7.bin").unwrap();
    let bios9 = fs::read("bios9.bin").unwrap();
    let firmware = fs::read("firmware.bin").unwrap();
    let rom = fs::read(rom_file).unwrap();
    let save_file = PathBuf::from(rom_file).with_extension("sav");
    let mut nds = NDS::new(bios7, bios9, firmware, rom, save_file);

    let mut palettes_window = DebugWindow::<PalettesWindowState>::new("Palettes");
    let mut maps_window = DebugWindow::<MapsWindowState>::new("Maps");
    let mut tiles_window = DebugWindow::<TilesWindowState>::new("Tiles");

    while !display.should_close() {
        nds.emulate_frame();
        
        let (keys_pressed, modifiers) = display.render_main(&mut nds, &mut imgui);
        display.render_imgui(&mut imgui, keys_pressed, modifiers,
            |ui, keys_pressed, _modifiers| {
            ui.main_menu_bar(|| {
                ui.menu(im_str!("Debug Windows"), true, || {
                    palettes_window.menu_item(ui);
                    maps_window.menu_item(ui);
                    tiles_window.menu_item(ui);
                });
            });

            palettes_window.render(&mut nds, ui, &keys_pressed);
            maps_window.render(&mut nds, ui, &keys_pressed);
            tiles_window.render(&mut nds, ui, &keys_pressed);
        });
    }
}
