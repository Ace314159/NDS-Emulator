use super::{GPU, Engine3D, math::Vec4, IORegister, Scheduler};

pub struct DISP3DCNT {
    texture_mapping: bool,
    highlight_shading: bool,
    alpha_test: bool,
    alpha_blending: bool,
    antia_aliasing: bool,
    edge_marking: bool,
    fog_alpha_only: bool,
    fog_master_enable: bool,
    fog_depth_shift: u8,
    color_buffer_underflow: bool,
    poly_vert_ram_overflow: bool,
    rear_plane_bitmap: bool,
}

impl DISP3DCNT {
    pub fn new() -> Self {
        DISP3DCNT {
            texture_mapping: false,
            highlight_shading: false,
            alpha_test: false,
            alpha_blending: false,
            antia_aliasing: false,
            edge_marking: false,
            fog_alpha_only: false,
            fog_master_enable: false,
            fog_depth_shift: 0,
            color_buffer_underflow: false,
            poly_vert_ram_overflow: false,
            rear_plane_bitmap: false,
        }
    }
}

impl IORegister for DISP3DCNT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.fog_master_enable as u8) << 7 | (self.fog_alpha_only as u8) << 6 | (self.edge_marking as u8) << 5 |
                (self.antia_aliasing as u8) << 4 | (self.alpha_blending as u8) << 3 | (self.alpha_test as u8) << 2 |
                (self.highlight_shading as u8) << 1 | self.texture_mapping as u8,
            1 => (self.rear_plane_bitmap as u8) << 6 | (self.poly_vert_ram_overflow as u8) << 5 |
                (self.color_buffer_underflow as u8) << 4 | self.fog_depth_shift,
            2 | 3 => 0,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => {
                self.texture_mapping = value >> 0 & 0x1 != 0;
                self.highlight_shading = value >> 1 & 0x1 != 0;
                self.alpha_test = value >> 2 & 0x1 != 0;
                self.alpha_blending = value >> 3 & 0x1 != 0;
                self.antia_aliasing = value >> 4 & 0x1 != 0;
                self.edge_marking = value >> 5 & 0x1 != 0;
                self.fog_alpha_only = value >> 6 & 0x1 != 0;
                self.fog_master_enable = value >> 7 & 0x1 != 0;
            },
            1 => {
                self.fog_depth_shift = value & 0xF;
                self.color_buffer_underflow = self.color_buffer_underflow && value >> 4 & 0x1 == 0;
                self.poly_vert_ram_overflow = self.poly_vert_ram_overflow && value >> 4 & 0x1 == 0;
                self.rear_plane_bitmap = value >> 6 & 0x1 != 0;
            },
            2 | 3 => (),
            _ => unreachable!(),
        }
    }
}

pub struct GXSTAT {
    pub test_busy: bool, // Box, Pos, Vector Test
    pub box_test_inside: bool,
    pub mat_stack_busy: bool,
    pub mat_stack_error: bool, // Overflow or Underflow
    pub geometry_engine_busy: bool,
    pub command_fifo_irq: CommandFifoIRQ,
}

#[derive(Clone, Copy)]
pub enum CommandFifoIRQ {
    Never = 0,
    LessHalf = 1,
    Empty = 2,
}

impl From<u8> for CommandFifoIRQ {
    fn from(value: u8) -> Self {
        match value {
            0 => CommandFifoIRQ::Never,
            1 => CommandFifoIRQ::LessHalf,
            2 => CommandFifoIRQ::Empty,
            3 => panic!("Reserved Command FIFO IRQ"),
            _ => unreachable!(),
        }
    }
}

impl GXSTAT {
    pub fn new() -> Self {
        GXSTAT {
            test_busy: false, // Box, Pos, Vector Test
            box_test_inside: false,
            mat_stack_busy: false,
            mat_stack_error: false, // Overflow or Underflow
            geometry_engine_busy: false,
            command_fifo_irq: CommandFifoIRQ::from(0),
        }
    }
}


impl Engine3D {
    pub(super) fn read_gxstat(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.gxstat.box_test_inside as u8) << 1| (self.gxstat.test_busy as u8),
            1 => (self.gxstat.mat_stack_error as u8) << 7 | (self.gxstat.mat_stack_busy as u8) << 6 |
                self.proj_stack_sp << 5 | self.pos_vec_stack_sp & 0x1F,
            2 => self.gxfifo.len() as u8,
            3 => (self.gxstat.command_fifo_irq as u8) << 6 | (self.gxstat.geometry_engine_busy as u8) << 3 |
                ((self.gxfifo.len() == 0) as u8) << 2 | ((self.gxfifo.len() < Engine3D::FIFO_LEN / 2) as u8) << 1 |
                (self.gxfifo.len() >> 8) as u8,
            _ => unreachable!(),
        }
    }

    pub(super) fn write_gxstat(&mut self, _scheduler: &mut crate::hw::scheduler::Scheduler, byte: usize, value: u8) {
        match byte {
            0 | 2 => (), // Read Only
            1 => self.gxstat.mat_stack_error = self.gxstat.mat_stack_error && value & 0x80 == 0,
            3 => self.gxstat.command_fifo_irq = CommandFifoIRQ::from(value >> 6 & 0x3),
            _ => unreachable!(),
        }
    }

    pub(super) fn read_ram_count(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.polygons.len() >> 0) as u8,
            1 => (self.polygons.len() >> 8) as u8,
            2 => (self.vertices.len() >> 0) as u8,
            3 => (self.vertices.len() >> 8) as u8,
            _ => unreachable!(),
        }
    }

    pub(super) fn read_clip_mat(&self, byte: usize) -> u8 {
        ((self.clip_mat[byte / 4].raw() as u32) >> (8 * (byte % 4))) as u8
    }
}

pub struct ClearColor {
    red: u8,
    green: u8,
    blue: u8,
    fog: bool,
    alpha: u8,
    polygon_id: u8,
}

impl ClearColor {
    pub fn new() -> Self {
        ClearColor {
            red: 0,
            green: 0,
            blue: 0,
            fog: false,
            alpha: 0,
            polygon_id: 0,
        }
    }

    pub fn color(&self) -> u16 {
        (self.blue as u16) << 10 | (self.green as u16) << 5 | self.red as u16
    }
}

impl IORegister for ClearColor {
    fn read(&self, _byte: usize) -> u8 { 0 }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => {
                self.red = value & 0x1F;
                self.green = self.green & !0x7 | (value >> 5) & 0x7;
            },
            1 => {
                self.green = self.green & !0x18 | (value << 3) & 0x18;
                self.blue = value >> 2 & 0x1F;
                self.fog = value >> 7 & 0x1 != 0;
            },
            2 => self.alpha = value & 0x1F,
            3 => self.polygon_id = value & 0x3F,
            _ => unreachable!(),
        }
    }
}

pub struct ClearDepth {
    depth: u16,
}

impl ClearDepth {
    pub fn new() -> Self {
        ClearDepth {
            depth: 0,
        }
    }

    pub fn depth(&self) -> u32 {
        (self.depth as u32) * 0x200 + 0x1FF
    }
}

impl IORegister for ClearDepth {
    fn read(&self, _byte: usize) -> u8 { 0 }
    
    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => self.depth = self.depth & !0xFF | value as u16,
            1 => self.depth = self.depth & !0x7F00 | (value as u16) << 8 & 0x7F00,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct TextureParams {
    pub vram_offset: usize,
    pub repeat_s: bool,
    pub repeat_t: bool,
    pub flip_s: bool,
    pub flip_t: bool,
    pub size_s: usize,
    pub size_t: usize,
    pub format: TextureFormat,
    pub color0_transparent: bool,
    pub coord_transformation_mode: TexCoordTransformationMode, 
}

impl TextureParams {
    pub fn new() -> Self {
        TextureParams {
            vram_offset: 0,
            repeat_s: false,
            repeat_t: false,
            flip_s: false,
            flip_t: false,
            size_s: 0,
            size_t: 0,
            format: TextureFormat::NoTexture,
            color0_transparent: false,
            coord_transformation_mode: TexCoordTransformationMode::None, 
        }
    }

    pub fn write(&mut self, value: u32) {
        self.vram_offset = ((value as usize) & 0xFFFF) << 3;
        self.repeat_s = value >> 16 & 0x1 != 0;
        self.repeat_t = value >> 17 & 0x1 != 0;
        self.flip_s = value >> 18 & 0x1 != 0;
        self.flip_t = value >> 19 & 0x1 != 0;
        self.size_s = 8 << (value >> 20 & 0x7);
        self.size_t = 8 << (value >> 23 & 0x7); 
        self.format = TextureFormat::from(value >> 26 & 0x7);
        self.color0_transparent = value >> 29 & 0x1 != 0;
        self.coord_transformation_mode = TexCoordTransformationMode::from(value >> 30 & 0x3);
    }
}

#[derive(Clone, Copy)]
pub enum TextureFormat {
    NoTexture = 0,
    A3I5 = 1,
    Palette4 = 2,
    Palette16 = 3,
    Palette256 = 4,
    Compressed = 5,
    A5I3 = 6,
    DirectColor = 7,
}

impl From<u32> for TextureFormat {
    fn from(value: u32) -> Self {
        match value {
            0 => TextureFormat::NoTexture,
            1 => TextureFormat::A3I5,
            2 => TextureFormat::Palette4,
            3 => TextureFormat::Palette16,
            4 => TextureFormat::Palette256,
            5 => TextureFormat::Compressed,
            6 => TextureFormat::A5I3,
            7 => TextureFormat::DirectColor,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum TexCoordTransformationMode {
    None = 0,
    TexCoord = 1,
    Normal = 2,
    Vertex = 3,
}

impl From<u32> for TexCoordTransformationMode {
    fn from(value: u32) -> Self {
        match value {
            0 => TexCoordTransformationMode::None,
            2 => TexCoordTransformationMode::Normal,
            1 => TexCoordTransformationMode::TexCoord,
            3 => TexCoordTransformationMode::Vertex,
            _ => todo!(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct PolygonAttributes {
    pub lights_enabled: [bool; 4],
    pub mode: PolygonMode,
    pub render_back: bool,
    pub render_front: bool,
    pub set_depth_translucent: bool,
    pub render_far_plane_intersecting: bool,
    pub render_1dot_behind_depth: bool,
    pub depth_test_eq: bool,
    pub fog_enable: bool,
    pub alpha: u8,
    pub polygon_id: u8,
}

impl PolygonAttributes {
    pub fn new() -> Self {
        PolygonAttributes {
            lights_enabled: [false; 4],
            mode: PolygonMode::Modulation,
            render_back: false,
            render_front: false,
            set_depth_translucent: false,
            render_far_plane_intersecting: false,
            render_1dot_behind_depth: false,
            depth_test_eq: false,
            fog_enable: false,
            alpha: 0,
            polygon_id: 0,
        }
    }

    pub fn write(&mut self, value: u32) {
        self.lights_enabled[0] = value >> 0 & 0x1 != 0;
        self.lights_enabled[1] = value >> 1 & 0x1 != 0;
        self.lights_enabled[2] = value >> 2 & 0x1 != 0;
        self.lights_enabled[3] = value >> 3 & 0x1 != 0;
        self.mode = PolygonMode::from(value >> 4 & 0x3);
        self.render_back = value >> 6 & 0x1 != 0;
        self.render_front = value >> 7 & 0x1 != 0;
        self.set_depth_translucent = value >> 11 & 0x1 != 0;
        self.render_far_plane_intersecting = value >> 12 & 0x1 != 0;
        self.render_1dot_behind_depth = value >> 13 & 0x1 != 0;
        self.depth_test_eq = value >> 14 & 0x1 != 0;
        self.fog_enable = value >> 15 & 0x1 != 0;
        self.alpha = (value >> 16 & 0x1F) as u8;
        self.polygon_id = (value >> 24 & 0x3F) as u8;
    }
}

#[derive(Clone, Copy)]
pub enum PolygonMode {
    Modulation = 0,
    Decal = 1,
    Toon = 2,
    Shadow = 3,
}

impl From<u32> for PolygonMode {
    fn from(value: u32) -> Self {
        match value {
            0 => PolygonMode::Modulation,
            1 => PolygonMode::Decal,
            2 => PolygonMode::Toon,
            3 => PolygonMode::Shadow,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct FrameParams {
    pub manual_sort_translucent: bool,
    pub w_buffer: bool,
}

impl FrameParams {
    pub fn new() -> Self {
        FrameParams {
            manual_sort_translucent: false,
            w_buffer: false,
        }
    }

    pub fn write(&mut self, value: u32) {
        self.manual_sort_translucent = (value >> 0) & 0x1 != 0;
        self.w_buffer = (value >> 1) & 0x1 != 0;
    }
}

pub struct Viewport {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    width: i32,
    height: i32,
}

impl Viewport {
    pub fn new() -> Self {
        Viewport {
            x1: 0,
            y1: 0,
            x2: 0,
            y2: 0,
            width: 0,
            height: 0,
        }
    }

    pub fn write(&mut self, value: u32) {
        self.x1 = (value >> 0) as u8 as i32;
        self.y1 = (value >> 8) as u8 as i32;
        self.x2 = (value >> 16) as u8 as i32;
        self.y2 = (value >> 24) as u8 as i32;
        assert!((self.y1 as usize) < GPU::HEIGHT);
        assert!((self.y2 as usize) < GPU::HEIGHT);
        self.width = self.x2 - self.x1 + 1;
        self.height = self.y2 - self.y1 + 1;
        assert!(self.width as usize <= GPU::WIDTH);
        assert!(self.height as usize <= GPU::HEIGHT);
    }

    pub fn screen_x(&self, clip_coords: &Vec4) -> usize {
        ((clip_coords[0].raw() + clip_coords[3].raw()) * self.width / (2 * clip_coords[3].raw()) + self.x1) as usize
    }

    pub fn screen_y(&self, clip_coords: &Vec4) -> usize {
        // Negate y because coords are flipped vertically
        ((-clip_coords[1].raw() + clip_coords[3].raw()) * self.height / (2 * clip_coords[3].raw()) + self.y1) as usize
    }
}

pub enum VertexPrimitive {
    Triangles = 0,
    Quad = 1,
    TriangleStrips = 2,
    QuadStrips = 3,
}

impl From<u32> for VertexPrimitive {
    fn from(value: u32) -> Self {
        match value {
            0 => VertexPrimitive::Triangles,
            1 => VertexPrimitive::Quad,
            2 => VertexPrimitive::TriangleStrips,
            3 => VertexPrimitive::QuadStrips,
            _ => unreachable!(),
        }
    }
}
