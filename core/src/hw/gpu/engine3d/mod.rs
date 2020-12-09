use std::collections::VecDeque;

use crate::hw::mmu::IORegister;
use super::{GPU, Scheduler, super::InterruptRequest};

mod registers;
mod math;
mod geometry;
mod rendering;

use math::{FixedPoint, Matrix};
use geometry::*;
use registers::*;

pub struct Engine3D {
    cycles_ahead: i32,
    pub bus_stalled: bool,
    // Registers
    gxstat: GXSTAT,
    // Geometry Engine
    prev_command: GeometryCommand, // Verification for Geometry Commands
    packed_commands: VecDeque<GeometryCommand>,
    num_params: usize,
    params_processed: usize,
    params: Vec<u32>,
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
    // Rendering Engine
    frame_params: FrameParams,
    next_frame_params: FrameParams,
    viewport: Viewport,
    clear_color: ClearColor,
    clear_depth: ClearDepth,
    pixels: Vec<u16>,
    depth_buffer: Vec<u32>,
    polygons_submitted: bool,
    // Polygons
    polygon_attrs: PolygonAttributes,
    polygon_attrs_latch: PolygonAttributes,
    vertex_primitive: VertexPrimitive,
    prev_pos: [FixedPoint; 3],
    swap_verts: bool,
    color: Color,
    cur_poly_verts: Vec<Vertex>,
    vertices: Vec<Vertex>,
    polygons: Vec<Polygon>,
    // Textures
    tex_params: TextureParams,
    palette_base: usize,
    raw_tex_coord: [i16; 2], // 1 + 11 + 4 fixed point
    tex_coord: [i16; 2], // 1 + 11 + 4 fixed point
}

impl Engine3D {
    const FIFO_LEN: usize = 256;
    const PIPE_LEN: usize = 4;

    pub fn new() -> Self {
        Engine3D {
            cycles_ahead: 0,
            bus_stalled: false,
            // Registers
            gxstat: GXSTAT::new(),
            // Geometry Engine
            prev_command: GeometryCommand::Unimplemented,
            packed_commands: VecDeque::new(),
            num_params: 0,
            params_processed: 0,
            params: Vec::new(),
            gxfifo: VecDeque::with_capacity(256),
            gxpipe: VecDeque::with_capacity(4),
            // Matrices
            mtx_mode: MatrixMode::Proj,
            cur_proj: Matrix::identity(),
            cur_pos: Matrix::identity(),
            cur_vec: Matrix::identity(),
            cur_tex: Matrix::identity(),
            proj_stack_sp: 0,
            pos_vec_stack_sp: 0,
            tex_stack_sp: 0,
            proj_stack: [Matrix::identity(); 1], // Projection Stack
            pos_stack: [Matrix::identity(); 31], // Coordinate Stack
            vec_stack: [Matrix::identity(); 31], // Directional Stack
            tex_stack: [Matrix::identity(); 1], // Texture Stack
            // Rendering Engine
            frame_params: FrameParams::new(),
            next_frame_params: FrameParams::new(),
            viewport: Viewport::new(),
            clear_color: ClearColor::new(),
            clear_depth: ClearDepth::new(),
            pixels: vec![0; GPU::WIDTH * GPU::HEIGHT],
            depth_buffer: vec![0; GPU::WIDTH * GPU::HEIGHT],
            polygons_submitted: false,
            // Polygons
            polygon_attrs: PolygonAttributes::new(),
            polygon_attrs_latch: PolygonAttributes::new(),
            vertex_primitive: VertexPrimitive::Triangles,
            prev_pos: [FixedPoint::zero(); 3],
            swap_verts: false,
            color: Color::new(0, 0, 0),
            cur_poly_verts: Vec::with_capacity(10),
            vertices: Vec::new(),
            polygons: Vec::new(),
            // Textures
            tex_params: TextureParams::new(),
            palette_base: 0,
            raw_tex_coord: [0; 2], // 1 + 11 + 4 fixed point
            tex_coord: [0; 2], // 1 + 11 + 4 fixed point
        }
    }

    pub fn clock(&mut self, cycles: usize, interrupts: &mut InterruptRequest) -> bool {
        if self.polygons_submitted {
            self.check_interrupts(interrupts);
            return false
        }
        self.cycles_ahead += cycles as i32;
        while self.cycles_ahead > 0 {
            if let Some(command_entry) = self.gxpipe.pop_front() {
                self.exec_command(command_entry);
                while self.gxpipe.len() < 3 {
                    if let Some(command_entry) = self.gxfifo.pop_front() {
                        self.gxpipe.push_back(command_entry);
                    } else { self.cycles_ahead = 0; break }
                }
            } else { self.cycles_ahead = 0; break }
        }
        self.check_interrupts(interrupts);
        self.gxfifo.len() < Engine3D::FIFO_LEN / 2
    }

    fn check_interrupts(&self, interrupts: &mut InterruptRequest) {
        if match self.gxstat.command_fifo_irq {
            CommandFifoIRQ::Never => false,
            CommandFifoIRQ::LessHalf => self.gxfifo.len() < Engine3D::FIFO_LEN / 2,
            CommandFifoIRQ::Empty => self.gxfifo.len() == 0,
        } { *interrupts |= InterruptRequest::GEOMETRY_COMMAND_FIFO }
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
            0x350 ..= 0x353 => self.clear_color.write(scheduler, addr as usize & 0x3, value),
            0x354 ..= 0x355 => self.clear_depth.write(scheduler, addr as usize & 0x1, value),
            0x600 ..= 0x603 => self.write_gxstat(scheduler, (addr as usize) & 0x3, value),
            _ => warn!("Ignoring Engine3D Write 0x{:08X} = {:02X}", addr, value),
        }
    }
}
