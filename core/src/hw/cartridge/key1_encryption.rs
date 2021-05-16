use std::convert::TryInto;

pub struct Key1Encryption {
    pub in_use: bool,
    key_buf: [u32; Key1Encryption::KEY_TABLE_SIZE],
    original_key_buf: [u32; Key1Encryption::KEY_TABLE_SIZE],
}

impl Key1Encryption {
    const KEY_TABLE_SIZE: usize = 0x1048 / 4;

    pub fn new(bios7: &[u8]) -> Self {
        let original_key_buf = bytemuck::cast_slice(&bios7[0x30..=0x1077]).try_into().unwrap();
        Key1Encryption {
            in_use: false,
            key_buf: original_key_buf,
            original_key_buf,
        }
    }

    // Modulo should be div by 4 before passing in
    pub fn init_key_code(&mut self, id_code: u32, level: u32, modulo: u32) {
        self.in_use = true;
        self.key_buf = self.original_key_buf;
        
        let mut key_code = [id_code, id_code / 2, id_code * 2];
        if level >= 1 { self.apply_keycode(&mut key_code, modulo) }
        if level >= 2 { self.apply_keycode(&mut key_code, modulo) }
        if level >= 3 {
            key_code[1] *= 2;
            key_code[2] /= 2;
            self.apply_keycode(&mut key_code, modulo)
        }
    }

    pub fn decrypt(&self, ptr: &mut [u32]) {
        Self::encrypt_decrypt64::<false>(&self.key_buf, ptr)
    }

    pub fn encrypt(&self, ptr: &mut [u32]) {
        Self::encrypt_decrypt64::<true>(&self.key_buf, ptr)
    }

    fn encrypt_decrypt64<const ENCRYPT: bool>(key_buf: &[u32], ptr: &mut [u32]) {
        let mut y = ptr[0 / 4];
        let mut x = ptr[4 / 4];

        let mut encrypt_range = 0x0..=0xF as usize;
        let mut decrypt_range = (0x2..=0x11 as usize).rev();
        let range = if ENCRYPT {
            &mut encrypt_range as &mut dyn Iterator<Item = _>
        } else { &mut decrypt_range };

        for i in range {
            let z = (key_buf[i] ^ x) as usize;
            x = key_buf[0x048 / 4 + (z >> 24 & 0xFF)];
            x = x.wrapping_add(key_buf[0x448 / 4 + (z >> 16 & 0xFF)]);
            x ^= key_buf[0x848 / 4 + (z >>  8 & 0xFF)];
            x = x.wrapping_add(key_buf[0xC48 / 4 + (z >> 0 & 0xFF)]);
            x ^= y;
            y = z as u32;
        }

        ptr[0 / 4] = x ^ if ENCRYPT { key_buf[0x40 / 4] } else { key_buf[0x4 / 4] };
        ptr[4 / 4] = y ^ if ENCRYPT { key_buf[0x44 / 4] } else { key_buf[0x0 / 4] };
    }

    fn encrypt64(key_buf: &[u32], ptr: &mut [u32]) {
        Self::encrypt_decrypt64::<true>(key_buf, ptr)
    }

    // Modulo should be div by 4 before passing in
    fn apply_keycode(&mut self, key_code: &mut [u32; 3], modulo: u32) {
        Self::encrypt64(&self.key_buf, &mut key_code[4 / 4..4 / 4 + 2]);
        Self::encrypt64(&self.key_buf, &mut key_code[0 / 4..0 / 4 + 2]);

        let mut scratch = [0, 0];
        for i in (0..=0x44 / 4).step_by(4 / 4) {
            self.key_buf[i] ^= key_code[i % modulo as usize].swap_bytes();
        }

        for i in (0..=0x1040 / 4).step_by(8 / 4) {
            Self::encrypt64(&self.key_buf, &mut scratch);
            self.key_buf[i] = scratch[4 / 4];
            self.key_buf[i + 4 / 4] = scratch[0 / 4];
        }
    }
}
