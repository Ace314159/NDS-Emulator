use crate::hw::mmu::IORegister;
use super::{GPU, Scheduler};

mod registers;

use registers::*;

pub struct Engine3D {
    gxstat: GXSTAT,
}

impl Engine3D {
    pub fn new() -> Self {
        Engine3D {
            gxstat: GXSTAT::new(),
        }
    }

    pub fn render_line(&self, line: &mut [u16; GPU::WIDTH]) {
        for pixel in line.iter_mut() { *pixel = 0x83F5 }
    }
}


impl Engine3D {
    pub fn read_register(&self, addr: u32) -> u8 {
        assert_eq!(addr >> 12, 0x04000);
        match addr & 0xFFF {
            0x600 ..= 0x603 => self.gxstat.read((addr as usize) & 0x3),
            _ => { warn!("Ignoring Engine3D Read at 0x{:08X}", addr); 0 },
        }
    }

    pub fn write_register(&mut self, scheduler: &mut Scheduler, addr: u32, value: u8) {
        assert_eq!(addr >> 12, 0x04000);
        match addr & 0xFFF {
            0x600 ..= 0x603 => self.gxstat.write(scheduler, (addr as usize) & 0x3, value),
            _ => warn!("Ignoring Engine3D Write 0x{:08X} = {:02X}", addr, value),
        }
    }
}
