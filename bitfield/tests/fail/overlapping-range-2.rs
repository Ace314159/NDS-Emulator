use bitfield::bitfield;

bitfield! {
    struct Bitfield: u8 {
        a: u8 @ 0..=7,
        b: bool @ 5,
    }
}

fn main() {
    
}
