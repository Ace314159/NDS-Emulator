use nds_core::nds::NDS;

fn main() {
    let mut nds = NDS::new();

    loop {
        nds.emulate_frame();
    }
}
