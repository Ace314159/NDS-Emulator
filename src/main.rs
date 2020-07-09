use std::fs;

use nds_core::simplelog::*;
use nds_core::nds::NDS;

fn main() {
    std::env::set_current_dir("ROMs").unwrap();
    CombinedLogger::init(vec![
        TermLogger::new(LevelFilter::Error, Config::default(), TerminalMode::Mixed),
        TermLogger::new(LevelFilter::Off,
            ConfigBuilder::new()
            .set_time_level(LevelFilter::Off)
            .set_thread_level(LevelFilter::Off)
            .set_target_level(LevelFilter::Off)
            .set_location_level(LevelFilter::Off)
            .set_time_level(LevelFilter::Off)
            .set_max_level(LevelFilter::Off)
            .add_filter_allow_str("nds_core::arm7")
            .build(),
            TerminalMode::Mixed),
    ]).unwrap();
    
    let bios7 = fs::read("bios7.bin").unwrap();
    let bios9 = fs::read("bios9.bin").unwrap();
    let rom = fs::read("armwrestler.nds").unwrap();
    let mut nds = NDS::new(bios7, bios9, rom);

    loop {
        nds.emulate_frame();
    }
}
