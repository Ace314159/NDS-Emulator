use super::{GPU, Engine3D, math::Vec4, IORegister, Scheduler};

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

pub struct TextureParams {
    vram_offset: usize,
    repeat_s: bool,
    repeat_t: bool,
    flip_s: bool,
    flip_t: bool,
    size_s: usize,
    size_t: usize,
    format: TextureFormat,
    color0_transparent: bool,
    coord_transformation_mode: TexCoordTransformationMode, 
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

pub enum TextureFormat {
    NoTexture = 0,
}

impl From<u32> for TextureFormat {
    fn from(value: u32) -> Self {
        match value {
            0 => TextureFormat::NoTexture,
            _ => todo!(),
        }
    }
}

pub enum TexCoordTransformationMode {
    None = 0,
}

impl From<u32> for TexCoordTransformationMode {
    fn from(value: u32) -> Self {
        match value {
            0 => TexCoordTransformationMode::None,
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
