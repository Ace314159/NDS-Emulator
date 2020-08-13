pub struct TSC {
    x: u16,
    y: u16,

    pos: usize,
    value: u16,
    return_byte: u8,
}

impl TSC {
    pub fn new() -> Self {
        TSC {
            x: 0,
            y: 0,

            pos: 0,
            value: 0,
            return_byte: 0,
        }
    }

    pub fn read(&self) -> u8 {
        self.return_byte
    }

    pub fn write(&mut self, value: u8) {
        self.return_byte = match self.pos {
            0 => self.value >> 5,
            1 => self.value << 3,
            _ => 0,
        } as u8;

        if value & 0x80 != 0 {
            let channel = value >> 4 & 0x7;
            self.pos = 0;
            self.value = match channel {
                1 => self.y,
                5 => self.x,
                6 => 0, // TODO: Microphone,
                _ => 0xFFF,
            };
        } else { self.pos += 1 }
    }

    pub fn deselect(&mut self) {
        self.pos = 0;
    }

    pub fn press_screen(&mut self, x: usize, y: usize) {
        self.x = (x as u16) << 4;
        self.y = (y as u16) << 4;
    }

    pub fn release_screen(&mut self) {
        self.x = 0;
        self.y = 0xFFF;
    }
}
