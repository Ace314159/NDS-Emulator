use bitfield::bitfield;

bitfield! {
    struct BasicBitfield: u16 {
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
    let mut basic_bitfield = BasicBitfield(0);

    // Check access
    assert_eq!(basic_bitfield.0, 0);
    assert_eq!(basic_bitfield.a(), false);
    assert_eq!(basic_bitfield.b(), false);
    assert_eq!(basic_bitfield.c(), 0);
    assert_eq!(basic_bitfield.d(), 0);
    assert_eq!(basic_bitfield.e(), false);
    assert_eq!(basic_bitfield.f(), 0);
    assert_eq!(basic_bitfield.g(), 0);
    assert_eq!(basic_bitfield.byte0(), 0);
    assert_eq!(basic_bitfield.byte1(), 0);

    // Set bits
    basic_bitfield.set_a(true);
    basic_bitfield.set_e(true);
    basic_bitfield.set_d(0b010);
    assert_eq!(basic_bitfield.0, 0b10100001);
    assert_eq!(basic_bitfield.a(), true);
    assert_eq!(basic_bitfield.b(), false);
    assert_eq!(basic_bitfield.c(), 0);
    assert_eq!(basic_bitfield.d(), 0b010);
    assert_eq!(basic_bitfield.e(), true);
    assert_eq!(basic_bitfield.f(), 0);
    assert_eq!(basic_bitfield.g(), 0);
    assert_eq!(basic_bitfield.byte0(), 0b10100001);
    assert_eq!(basic_bitfield.byte1(), 0);

    // Byte operations
    basic_bitfield.set_byte1(0xAA);
    assert_eq!(basic_bitfield.0, 0b1010101010100001);
    assert_eq!(basic_bitfield.a(), true);
    assert_eq!(basic_bitfield.b(), false);
    assert_eq!(basic_bitfield.c(), 0);
    assert_eq!(basic_bitfield.d(), 0b010);
    assert_eq!(basic_bitfield.e(), true);
    assert_eq!(basic_bitfield.f(), 0b01010);
    assert_eq!(basic_bitfield.g(), 0b101);
    assert_eq!(basic_bitfield.byte0(), 0b10100001);
    assert_eq!(basic_bitfield.byte1(), 0xAA);
}

bitfield! {
    struct SkippedBitfield: u8 {
        _: bool, 0,
        a: bool, 1,
        _: u8, 2..=5,
        b: u8, 6..=7,
    }
}

#[test]
fn fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/fail/*.rs");
}
