use bitfield::bitfield;

bitfield! {
    struct Bitfield: u8 {
        a: u8 @ 0..=5,
        b: bool @ 6..=7,
    }
}

fn main() {
    
}
