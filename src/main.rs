mod display;
mod debug;

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

    let mut palettes_window = TextureWindow::new("Palettes");

    while !display.should_close() {
        nds.emulate_frame();
        let (pixels, width, height) = nds.render_palettes(Engine::A, GraphicsType::BG);
        display.render(&mut nds, &mut imgui,
            |ui, keys_pressed, _modifiers| {
            palettes_window.render(ui, &keys_pressed, pixels, width, height, || {});
        });
    }
}
