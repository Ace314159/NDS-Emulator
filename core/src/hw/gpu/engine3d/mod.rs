use std::collections::VecDeque;

use super::{GPU, Scheduler, Event};

mod registers;
mod geometry;

pub use geometry::GeometryCommandEntry;

use geometry::*;
use registers::*;

pub struct Engine3D {
    // Registers
    gxstat: GXSTAT,
    // Geometry Engine
    gxfifo: VecDeque<GeometryCommandEntry>,
    gxpipe: VecDeque<GeometryCommandEntry>,
    // Matrices
    mtx_mode: MatrixMode,
    cur_proj: Matrix,
    cur_pos: Matrix,
    cur_vec: Matrix,
    cur_tex: Matrix,
    proj_stack_sp: u8,
    pos_vec_stack_sp: u8,
    tex_stack_sp: u8,
    proj_stack: [Matrix; 1], // Projection Stack
    pos_stack: [Matrix; 31], // Coordinate Stack
    vec_stack: [Matrix; 31], // Directional Stack
    tex_stack: [Matrix; 1], // Texture Stack
}

impl Engine3D {
    const FIFO_LEN: usize = 256;
    const PIPE_LEN: usize = 4;

    pub fn new() -> Self {
        Engine3D {
            // Registers
            gxstat: GXSTAT::new(),
            // Geometry Engine
            gxfifo: VecDeque::with_capacity(256),
            gxpipe: VecDeque::with_capacity(4),
            // Matrices
            mtx_mode: MatrixMode::Proj,
            cur_proj: Matrix::empty(),
            cur_pos: Matrix::empty(),
            cur_vec: Matrix::empty(),
            cur_tex: Matrix::empty(),
            proj_stack_sp: 0,
            pos_vec_stack_sp: 0,
            tex_stack_sp: 0,
            proj_stack: [Matrix::empty(); 1], // Projection Stack
            pos_stack: [Matrix::empty(); 31], // Coordinate Stack
            vec_stack: [Matrix::empty(); 31], // Directional Stack
            tex_stack: [Matrix::empty(); 1], // Texture Stack
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
            0x600 ..= 0x603 => self.read_gxstat((addr as usize) & 0x3),
            _ => { warn!("Ignoring Engine3D Read at 0x{:08X}", addr); 0 },
        }
    }

    pub fn write_register(&mut self, scheduler: &mut Scheduler, addr: u32, value: u8) {
        assert_eq!(addr >> 12, 0x04000);
        match addr & 0xFFF {
            0x600 ..= 0x603 => self.write_gxstat(scheduler, (addr as usize) & 0x3, value),
            _ => warn!("Ignoring Engine3D Write 0x{:08X} = {:02X}", addr, value),
        }
    }
}
