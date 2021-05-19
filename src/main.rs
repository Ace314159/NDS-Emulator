mod debug;
mod display;

use std::fs;
use std::path::{PathBuf, Path};

use nds_core::log::*;
use nds_core::nds::{Engine, GraphicsType, NDS};
use nds_core::simplelog::*;

use debug::*;
use display::Display;
use imgui::*;

fn main() {
    let args: Vec<_> = std::env::args().collect();

    if args.len() != 2 {
        println!("Usage: {} <ROM file>", args[0]);
        std::process::exit(1);
    }

    let rom_path = Path::new(&args[1]);
    let bios7_path = PathBuf::from("ROMs/bios7.bin");
    let bios9_path = PathBuf::from("ROMs/bios9.bin");
    let firmware_path = PathBuf::from("ROMs/firmware.bin");

    let arm7_file_name = "ROMs/arm7.log";
    let arm9_file_name = "ROMs/arm9.log";
    let instructions7_filter = LevelFilter::Off;
    let instructions9_filter = LevelFilter::Off;

    let arm7_file = fs::File::create(arm7_file_name);
    let arm9_file = fs::File::create(arm9_file_name);
    let mut loggers: Vec<Box<dyn SharedLogger>> = vec![TermLogger::new(
        LevelFilter::Warn,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )];
    if let Ok(file) = arm7_file {
        loggers.push(WriteLogger::new(
            instructions7_filter,
            ConfigBuilder::new()
                .set_time_level(LevelFilter::Off)
                .set_thread_level(LevelFilter::Off)
                .set_target_level(LevelFilter::Off)
                .set_location_level(LevelFilter::Off)
                .set_time_level(LevelFilter::Off)
                .set_max_level(LevelFilter::Off)
                .add_filter_allow_str("nds_core::arm7")
                .build(),
            file,
        ));
    }
    if let Ok(file) = arm9_file {
        loggers.push(WriteLogger::new(
            instructions9_filter,
            ConfigBuilder::new()
                .set_time_level(LevelFilter::Off)
                .set_thread_level(LevelFilter::Off)
                .set_target_level(LevelFilter::Off)
                .set_location_level(LevelFilter::Off)
                .set_time_level(LevelFilter::Off)
                .set_max_level(LevelFilter::Off)
                .add_filter_allow_str("nds_core::arm9")
                .build(),
            file,
        ))
    }
    CombinedLogger::init(loggers).unwrap();

    let mut nds = load_rom(&bios7_path, &bios9_path, &firmware_path, rom_path);

    let mut main_menu_height = 0.0;
    let mut palettes_window = DebugWindow::<PalettesWindowState>::new("Palettes");
    let mut maps_window = DebugWindow::<MapsWindowState>::new("Maps");
    let mut tiles_window = DebugWindow::<TilesWindowState>::new("Tiles");
    let mut vram_window = DebugWindow::<VRAMWindowState>::new("VRAM");
    let mut stats_window = StatsWindow::new();

    let mut imgui = Context::create();
    let mut display = Display::new(&mut imgui);

    let main_loop = move |display: &mut Display| {
        nds.emulate_frame();
        stats_window.frame_completed();

        let (keys_pressed, files_dropped) =
            display.render_main(&mut nds, &mut imgui, main_menu_height);
        display.render_imgui(&mut imgui, keys_pressed, |ui, keys_pressed| {
            ui.main_menu_bar(|| {
                ui.menu(im_str!("Debug Windows"), true, || {
                    palettes_window.menu_item(ui);
                    maps_window.menu_item(ui);
                    tiles_window.menu_item(ui);
                    vram_window.menu_item(ui);
                    stats_window.menu_item(ui);
                });
                main_menu_height = ui.window_size()[1];
            });

            palettes_window.render(&mut nds, ui, &keys_pressed);
            maps_window.render(&mut nds, ui, &keys_pressed);
            tiles_window.render(&mut nds, ui, &keys_pressed);
            vram_window.render(&mut nds, ui, &keys_pressed);
            stats_window.render(ui);
        });

        if files_dropped.len() == 1 {
            if let Some(ext) = files_dropped[0].extension() {
                if let Some(str) = ext.to_str() {
                    if str.to_lowercase() == "nds" {
                        nds = load_rom(&bios7_path, &bios9_path, &firmware_path, &files_dropped[0]);
                    } else {
                        error!("File is not a .nds file!")
                    }
                }
            } else {
                error!("File does not have an extension!")
            }
        } else if files_dropped.len() > 1 {
            error!("More than 1 file dropped!")
        }
    };

    display.run_main_loop(main_loop);

    fn load_rom(
        bios7_path: &PathBuf,
        bios9_path: &PathBuf,
        firmware_path: &PathBuf,
        rom_path: &Path,
    ) -> NDS {
        NDS::new(
            fs::read(bios7_path).unwrap(),
            fs::read(bios9_path).unwrap(),
            fs::read(firmware_path).unwrap(),
            fs::read(rom_path).unwrap(),
            rom_path.with_extension("sav"),
        )
    }
}
