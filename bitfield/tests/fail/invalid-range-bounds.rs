use bitfield::bitfield;

bitfield! {
    struct Bitfield: u8 {
        a: u8 @ 6..=0,
        b: u8 @ 7,
    }
}

fn main () {
    
}
