use super::{ARM9, HW};

pub(super) type InstructionHandler<T> = fn(&mut ARM9, &mut HW, T);

macro_rules! compose_instr_handler {
    ($handler: ident, $skeleton: expr, $($bit: expr),* ) => {
        compose_instr_handler!($handler, flags => (), values => ( $($skeleton >> $bit & 0x1 != 0),* ))
    };
    ($handler: ident, flags => ( $( $flag: expr),* ), values => ()) => {
        ARM9::$handler::<$($flag,)*>
    };
    ($handler: ident, flags => ( $($flag: expr),* ), values => ( $cur_value:expr $( , $value: expr )* )) => {
        if $cur_value {
            compose_instr_handler!($handler, flags => ( $($flag,)* true ), values => ( $($value),* ))
        } else {
            compose_instr_handler!($handler, flags => ( $($flag,)* false ), values => ( $($value),* ))
        }
    };
}

pub(super) const fn gen_condition_table() -> [bool; 256] {
    let mut lut = [false; 256];
    let (n_mask, z_mask, c_mask, v_mask) = (0x8, 0x4, 0x2, 0x1);
    // TODO: Replace with for loop?
    let mut flags = 0;
    while flags <= 0xF {
        let mut condition = 0;
        while condition <= 0xF {
            let n = flags & n_mask != 0;
            let z = flags & z_mask != 0;
            let c = flags & c_mask != 0;
            let v = flags & v_mask != 0;
            lut[flags << 4 | condition] = match condition {
                0x0 => z,
                0x1 => !z,
                0x2 => c,
                0x3 => !c,
                0x4 => n,
                0x5 => !n,
                0x6 => v,
                0x7 => !v,
                0x8 => c && !z,
                0x9 => !c || z,
                0xA => n == v,
                0xB => n != v,
                0xC => !z && n == v,
                0xD => z || n != v,
                0xE => true,
                0xF => true, // True so that some ARMv5 instructions can execute
                _ => true,   // TODO: Add unreachable!()
            };
            condition += 1;
        }
        flags += 1;
    }

    lut
}
