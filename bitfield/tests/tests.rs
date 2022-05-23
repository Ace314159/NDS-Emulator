use bitfield::bitfield;

bitfield! {
    struct Bitfield: u16 {
        a: bool, 0,
        b: bool, 1,
        c: u8, 2..=3,
        d: u8, 4..=6,
        e: bool, 7,
        f: u8, 8..=12,
        g: u8, 13..=15,
    }
}

#[test]
fn basic_usage() {
    let mut bitfield = Bitfield(0);

    // Check access
    assert_eq!(bitfield.0, 0);
    assert_eq!(bitfield.a(), false);
    assert_eq!(bitfield.b(), false);
    assert_eq!(bitfield.c(), 0);
    assert_eq!(bitfield.d(), 0);
    assert_eq!(bitfield.e(), false);
    assert_eq!(bitfield.f(), 0);
    assert_eq!(bitfield.g(), 0);
    assert_eq!(bitfield.byte0(), 0);
    assert_eq!(bitfield.byte1(), 0);

    // Set bits
    bitfield.set_a(true);
    bitfield.set_e(true);
    bitfield.set_d(0b010);
    assert_eq!(bitfield.0, 0b10100001);
    assert_eq!(bitfield.a(), true);
    assert_eq!(bitfield.b(), false);
    assert_eq!(bitfield.c(), 0);
    assert_eq!(bitfield.d(), 0b010);
    assert_eq!(bitfield.e(), true);
    assert_eq!(bitfield.f(), 0);
    assert_eq!(bitfield.g(), 0);
    assert_eq!(bitfield.byte0(), 0b10100001);
    assert_eq!(bitfield.byte1(), 0);

    // Byte operations
    bitfield.set_byte1(0xAA);
    assert_eq!(bitfield.0, 0b1010101010100001);
    assert_eq!(bitfield.a(), true);
    assert_eq!(bitfield.b(), false);
    assert_eq!(bitfield.c(), 0);
    assert_eq!(bitfield.d(), 0b010);
    assert_eq!(bitfield.e(), true);
    assert_eq!(bitfield.f(), 0b01010);
    assert_eq!(bitfield.g(), 0b101);
    assert_eq!(bitfield.byte0(), 0b10100001);
    assert_eq!(bitfield.byte1(), 0xAA);
}

#[test]
fn fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/fail/*.rs");
}
