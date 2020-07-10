mod display;

use std::fs;

use nds_core::simplelog::*;
use nds_core::nds::NDS;

use display::Display;
use imgui::*;

fn main() {
    std::env::set_current_dir("ROMs").unwrap();
    CombinedLogger::init(vec![
        TermLogger::new(LevelFilter::Warn, Config::default(), TerminalMode::Mixed),
        WriteLogger::new(LevelFilter::Off,
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
        WriteLogger::new(LevelFilter::Off,
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
    let rom = fs::read("armwrestler.nds").unwrap();
    let mut nds = NDS::new(bios7, bios9, rom);

    while !display.should_close() {
        nds.emulate_frame();
        display.render(nds.get_screens(), &mut imgui,
            |_ui, _keys_pressed, _modifiers| {

        });
    }
}
