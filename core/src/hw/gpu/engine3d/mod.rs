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
    pub bus_stalled: bool,
    // Registers
    pub disp3dcnt: DISP3DCNT,
    gxstat: GXSTAT,
    // Geometry Engine
    prev_command: GeometryCommand, // Verification for Geometry Commands
    packed_commands: u32,
    cur_command: GeometryCommand, // Current Packed Command processing
    num_params: usize,
    params_processed: usize,
    params: Vec<u32>,
    gxfifo: VecDeque<GeometryCommandEntry>,
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
    clip_mat: Matrix,
    cur_poly_verts: Vec<Vertex>,
    vertices: Vec<Vertex>,
    polygons: Vec<Polygon>,
    original_verts: Vec<(Matrix, [FixedPoint; 3])>,
    // Lighting
    lights: [Light; 4],
    material: Material,
    color: Color,
    // Textures
    tex_params: TextureParams,
    palette_base: usize,
    raw_tex_coord: [i16; 2], // 1 + 11 + 4 fixed point
    tex_coord: [i16; 2], // 1 + 11 + 4 fixed point
    // Toon
    toon_table: [Color; 0x20],
}

impl Engine3D {
    const FIFO_LEN: usize = 256;

    pub fn new() -> Self {
        Engine3D {
            bus_stalled: false,
            // Registers
            disp3dcnt: DISP3DCNT::new(),
            gxstat: GXSTAT::new(),
            // Geometry Engine
            prev_command: GeometryCommand::Unimplemented,
            packed_commands: 0,
            cur_command: GeometryCommand::Unimplemented,
            num_params: 0,
            params_processed: 0,
            params: Vec::new(),
            gxfifo: VecDeque::with_capacity(256),
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
            clip_mat: Matrix::identity(),
            cur_poly_verts: Vec::with_capacity(10),
            vertices: Vec::new(),
            polygons: Vec::new(),
            original_verts: Vec::new(),
            // Lighting
            lights: [Light::new(); 4],
            material: Material::new(),
            color: Color::new5(0, 0, 0),
            // Textures
            tex_params: TextureParams::new(),
            palette_base: 0,
            raw_tex_coord: [0; 2], // 1 + 11 + 4 fixed point
            tex_coord: [0; 2], // 1 + 11 + 4 fixed point
            // Toon
            toon_table: [Color::new5(0, 0, 0); 0x20],
        }
    }

    pub fn clock(&mut self, interrupts: &mut InterruptRequest) -> bool {
        self.check_interrupts(interrupts);
        if self.polygons_submitted {
            false
        } else {
            while let Some(entry) = self.gxfifo.pop_front() {
                self.exec_command(entry);
                if self.polygons_submitted { break }
            }
            self.gxfifo.len() < Engine3D::FIFO_LEN / 2
        }
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
            0x4A4 ..= 0x4A7 => 0, // TODO: Figure out what this should actually do
            0x600 ..= 0x603 => self.read_gxstat((addr as usize) & 0x3),
            0x604 ..= 0x607 => self.read_ram_count((addr as usize) & 0x3),
            0x640 ..= 0x67F => self.read_clip_mat((addr as usize) & 0x3F),
            _ => { warn!("Ignoring Engine3D Read at 0x{:08X}", addr); 0 },
        }
    }

    pub fn write_register(&mut self, scheduler: &mut Scheduler, addr: u32, value: u8) {
        assert_eq!(addr >> 12, 0x04000);
        match addr & 0xFFF {
            0x350 ..= 0x353 => self.clear_color.write(scheduler, addr as usize & 0x3, value),
            0x354 ..= 0x355 => self.clear_depth.write(scheduler, addr as usize & 0x1, value),
            0x380 ..= 0x3BF => self.write_toon_table(addr as usize & (2 * self.toon_table.len() - 1), value),
            0x600 ..= 0x603 => self.write_gxstat(scheduler, (addr as usize) & 0x3, value),
            _ => warn!("Ignoring Engine3D Write 0x{:08X} = {:02X}", addr, value),
        }
    }
}
