use bitfield::bitfield;

bitfield! {
    struct Bitfield: u8 {
        a: u8, 0..=6,
        b: u8, 7..=7,
    }
}

fn main() {
    
}
