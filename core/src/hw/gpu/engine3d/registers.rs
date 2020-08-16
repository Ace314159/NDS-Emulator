use super::{Engine3D, IORegister, Scheduler};

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
