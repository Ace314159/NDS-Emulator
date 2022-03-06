use std::path::{Path, PathBuf};

use criterion::{criterion_group, criterion_main, Criterion};
use nds_core::NDS;

fn bench(c: &mut Criterion) {
    let rom_path = Path::new("ROMs/game.nds");
    let bios7_path = PathBuf::from("ROMs/bios7.bin");
    let bios9_path = PathBuf::from("ROMs/bios9.bin");
    let firmware_path = PathBuf::from("ROMs/firmware.bin");

    c.bench_function("FirstSecond", |b| {
        b.iter_batched(
            || NDS::load_rom(&bios7_path, &bios9_path, &firmware_path, rom_path),
            |mut nds| {
                for _ in 0..60 {
                    nds.emulate_frame();
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
