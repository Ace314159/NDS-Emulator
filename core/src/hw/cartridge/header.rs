use std::convert::TryInto;

pub struct Header {
    pub game_title: [u8; 12], // ASCII
    pub game_code: [u8; 4],   // ASCII - 0 = homebrew
    pub maker_code: [u8; 2],  // ASCII - 0 = homebrew
    pub unit_code: UnitCode,
    pub encryption_seed: u8, // 0x0 - 0x7
    pub device_capacity: u8, // 0x2_0000 << nn
    pub reserved0: [u8; 7],  // 0 filled
    pub reserved1: u8,       // 0 unless DSi
    pub region: Region,
    pub rom_version: u8,
    pub autostart: u8, // Bit 2
    pub arm9_rom_offset: u32,
    pub arm9_entry_addr: u32, // 0x0200_0000 - 0x023B_FE00
    pub arm9_ram_addr: u32,   // 0x0200_0000 - 0x023B_FE00
    pub arm9_size: u32,       // Max 0x3BFE00
    pub arm7_rom_offset: u32, // 0x8000 and up
    pub arm7_entry_addr: u32, // 0x0200_0000 - 0x023B_FE00 or 0x037F_8000 - 0x0380_7E00
    pub arm7_ram_addr: u32,   // 0x0200_0000 - 0x023B_FE00 or 0x037F_8000 - 0x0380_7E00
    pub arm7_size: u32,       // Max 0x3B_FE00 or 0xFE00
    pub fnt_offset: u32,
    pub fnt_size: u32,
    pub fat_offset: u32,
    pub fat_size: u32,
    pub arm9_overlay_offset: u32,
    pub arm9_overlay_size: u32,
    pub arm7_overlay_offset: u32,
    pub arm7_overlay_size: u32,
    pub port_settings_normal: u32,
    pub port_settings_key1: u32,
    pub icon_offset: u32,          // 0x8000 and up
    pub secure_area_checksum: u16, // CRC-16 of [[0x20] - 0x0000_7FFF]
    pub secure_area_delay: u16,
    pub arm9_auto_load_list_hook_ram_addr: u32,
    pub arm7_auto_load_list_hook_ram_addr: u32,
    pub secure_area_disable: u64,
    pub used_rom_size: u32,    // Unused Usually 0xFF padded
    pub header_size: u32,      // 0x4000
    pub reserved2: [u8; 0x28], // 0 unless DSi where first 11 bytes used
    pub reserved3: [u8; 0x10], // 0
    pub nintendo_logo: [u8; 0x9C],
    pub nintendo_logo_checksum: u16, // 0xCF56
    pub header_checksum: u16,        // CRC-16 0x000 - 0x15D
                                     // pub debug_rom_offset: u32, // 0 = None, 0x8000 and up
                                     // pub debug_size: u32, // 0 = None, Max 0x3B_FE00
                                     // pub debug_ram_addr: u32, // 0 = None, 0x0240_0000..0x027B_FE00
                                     // pub reserved4: [u8; 4], // 0 - Transferred and stored, but not used
                                     // pub reserved5: [u8; 0x90], // 0 - Transferred but not stored in RAM
}

impl Header {
    pub fn new(rom: &Vec<u8>) -> Header {
        Header {
            game_title: rom[0x000..0x00C].try_into().unwrap(),
            game_code: rom[0x00C..0x010].try_into().unwrap(),
            maker_code: rom[0x010..0x012].try_into().unwrap(),
            unit_code: UnitCode::from_byte(rom[0x012]),
            encryption_seed: rom[0x013],
            device_capacity: rom[0x014],
            reserved0: rom[0x015..0x01C].try_into().unwrap(),
            reserved1: rom[0x01C],
            region: Region::from_byte(rom[0x01D]),
            rom_version: rom[0x01E],
            autostart: rom[0x01F],
            arm9_rom_offset: u32::from_le_bytes(rom[0x020..0x024].try_into().unwrap()),
            arm9_entry_addr: u32::from_le_bytes(rom[0x024..0x028].try_into().unwrap()),
            arm9_ram_addr: u32::from_le_bytes(rom[0x028..0x02C].try_into().unwrap()),
            arm9_size: u32::from_le_bytes(rom[0x02C..0x030].try_into().unwrap()),
            arm7_rom_offset: u32::from_le_bytes(rom[0x030..0x034].try_into().unwrap()),
            arm7_entry_addr: u32::from_le_bytes(rom[0x034..0x038].try_into().unwrap()),
            arm7_ram_addr: u32::from_le_bytes(rom[0x038..0x03C].try_into().unwrap()),
            arm7_size: u32::from_le_bytes(rom[0x03C..0x040].try_into().unwrap()),
            fnt_offset: u32::from_le_bytes(rom[0x040..0x044].try_into().unwrap()),
            fnt_size: u32::from_le_bytes(rom[0x044..0x048].try_into().unwrap()),
            fat_offset: u32::from_le_bytes(rom[0x048..0x04C].try_into().unwrap()),
            fat_size: u32::from_le_bytes(rom[0x04C..0x050].try_into().unwrap()),
            arm9_overlay_offset: u32::from_le_bytes(rom[0x050..0x054].try_into().unwrap()),
            arm9_overlay_size: u32::from_le_bytes(rom[0x054..0x058].try_into().unwrap()),
            arm7_overlay_offset: u32::from_le_bytes(rom[0x058..0x05C].try_into().unwrap()),
            arm7_overlay_size: u32::from_le_bytes(rom[0x05C..0x060].try_into().unwrap()),
            port_settings_normal: u32::from_le_bytes(rom[0x060..0x064].try_into().unwrap()),
            port_settings_key1: u32::from_le_bytes(rom[0x064..0x068].try_into().unwrap()),
            icon_offset: u32::from_le_bytes(rom[0x068..0x06C].try_into().unwrap()),
            secure_area_checksum: u16::from_le_bytes(rom[0x06C..0x06E].try_into().unwrap()),
            secure_area_delay: u16::from_le_bytes(rom[0x06E..0x070].try_into().unwrap()),
            arm9_auto_load_list_hook_ram_addr: u32::from_le_bytes(
                rom[0x070..0x074].try_into().unwrap(),
            ),
            arm7_auto_load_list_hook_ram_addr: u32::from_le_bytes(
                rom[0x074..0x078].try_into().unwrap(),
            ),
            secure_area_disable: u64::from_le_bytes(rom[0x078..0x080].try_into().unwrap()),
            used_rom_size: u32::from_le_bytes(rom[0x080..0x084].try_into().unwrap()),
            header_size: u32::from_le_bytes(rom[0x084..0x088].try_into().unwrap()),
            reserved2: rom[0x088..0x0B0].try_into().unwrap(),
            reserved3: rom[0x0B0..0x0C0].try_into().unwrap(),
            nintendo_logo: rom[0x0C0..0x15C].try_into().unwrap(),
            nintendo_logo_checksum: u16::from_le_bytes(rom[0x15C..0x15E].try_into().unwrap()),
            header_checksum: u16::from_le_bytes(rom[0x15E..0x160].try_into().unwrap()),
            // debug_rom_offset: u32::from_le_bytes(rom[0x160..0x164].try_into().unwrap()),
            // debug_size: u32::from_le_bytes(rom[0x164..0x168].try_into().unwrap()),
            // debug_ram_addr: u32::from_le_bytes(rom[0x168..0x16C].try_into().unwrap()),
            // reserved4: rom[0x16C..0x170].try_into().unwrap(),
            // reserved5: rom[0x170..0x200].try_into().unwrap(),
        }
    }
}

pub enum UnitCode {
    NDS,
    Both,
    DSi,
}

impl UnitCode {
    pub fn from_byte(value: u8) -> Self {
        match value {
            0b00 => UnitCode::NDS,
            0b10 => UnitCode::Both,
            0b11 => UnitCode::DSi,
            _ => unreachable!(),
        }
    }
}

pub enum Region {
    Normal,
    China,
    Korea,
}

impl Region {
    pub fn from_byte(value: u8) -> Self {
        match value {
            0x00 => Region::Normal,
            0x80 => Region::China,
            0x40 => Region::Korea,
            _ => unreachable!(),
        }
    }
}
