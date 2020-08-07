use bitflags::*;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use crate::hw::{
    mmu::IORegister,
    scheduler::Scheduler,
};
use super::EngineType;

#[derive(Clone, Copy, PartialEq)]
pub enum BGMode {
    Mode0 = 0,
    Mode1 = 1,
    Mode2 = 2,
    Mode3 = 3,
    Mode4 = 4,
    Mode5 = 5,
    Mode6 = 6,
}

impl BGMode {
    pub fn from_bits(bits: u8) -> Self {
        use BGMode::*;
        match bits {
            0 => Mode0,
            1 => Mode1,
            2 => Mode2,
            3 => Mode3,
            4 => Mode4,
            5 => Mode5,
            6 => Mode6,
            _ => panic!("Invalid BG Mode!"),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum DisplayMode {
    Mode0 = 0,
    Mode1 = 1,
    Mode2 = 2,
    Mode3 = 3,
}

impl DisplayMode {
    pub fn from_bits(bits: u8) -> Self {
        use DisplayMode::*;
        match bits {
            0 => Mode0,
            1 => Mode1,
            2 => Mode2, // TODO: Only Engine A
            3 => Mode3, // TODO: Only Engine A
            _ => unreachable!(),
        }
    }
}

bitflags! {
    pub struct DISPCNTFlags: u32 {
        const IS_3D = 1 << 3;
        const TILE_OBJ_1D = 1 << 4;
        const BITMAP_OBJ_SQUARE = 1 << 5;
        const BITMAP_OBJ_1D = 1 << 6;
        const FORCED_BLANK = 1 << 7;
        const DISPLAY_BG0 = 1 << 8;
        const DISPLAY_BG1 = 1 << 9;
        const DISPLAY_BG2 = 1 << 10;
        const DISPLAY_BG3 = 1 << 11;
        const DISPLAY_OBJ = 1 << 12;
        const DISPLAY_WINDOW0 = 1 << 13;
        const DISPLAY_WINDOW1 = 1 << 14;
        const DISPLAY_OBJ_WINDOW = 1 << 15;
        const BITMAP_OBJ_1D_BOUND = 1 << 22;
        const OBJ_PROCESS_HBLANK = 1 << 23;
        const BG_EXTENDED_PALETTES = 1 << 30;
        const OBJ_EXTENDED_PALETTES = 1 << 31;
    }
}

pub struct DISPCNT<E: EngineType> {
    pub flags: DISPCNTFlags,
    pub bg_mode: BGMode,
    pub display_mode: DisplayMode,
    pub vram_block: u8,
    pub tile_obj_1d_bound: u8,
    pub char_base: u8,
    pub screen_base: u8,
    engine_type: PhantomData<E>,
}

impl<E: EngineType> DISPCNT<E> {
    pub fn new() -> DISPCNT<E> {
        DISPCNT {
            flags: DISPCNTFlags::empty(),
            bg_mode: BGMode::Mode0,
            display_mode: DisplayMode::Mode0,
            vram_block: 0,
            tile_obj_1d_bound: 0,
            char_base: 0,
            screen_base: 0,
            engine_type: PhantomData,
        }
    }
    
    pub fn windows_enabled(&self) -> bool {
        (self.bits >> 13) & 0x7 != 0
    }
}

impl<E: EngineType> Deref for DISPCNT<E> {
    type Target = DISPCNTFlags;

    fn deref(&self) -> &DISPCNTFlags {
        &self.flags
    }
}

impl<E: EngineType> DerefMut for DISPCNT<E> {
    fn deref_mut(&mut self) -> &mut DISPCNTFlags {
        &mut self.flags
    }
}

impl<E: EngineType> IORegister for DISPCNT<E> {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.flags.bits >> 0) as u8 | self.bg_mode as u8,
            1 => (self.flags.bits >> 8) as u8,
            2 => (self.flags.bits >> 16) as u8 | self.tile_obj_1d_bound << 4 | self.vram_block << 2 | self.display_mode as u8,
            3 => (self.flags.bits >> 24) as u8 | self.screen_base << 3 | self.char_base,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => {
                self.bg_mode = BGMode::from_bits(value & 0x7);
                self.flags.bits = self.flags.bits & !0x0000_00FF | (value as u32) & DISPCNTFlags::all().bits; 
                // TODO: Use trait specialization instead of this
                if !E::is_a() {
                    self.flags.remove(DISPCNTFlags::IS_3D);
                }
            },
            1 => self.flags.bits = self.flags.bits & !0x0000_FF00 | (value as u32) << 8 & DISPCNTFlags::all().bits,
            2 => {
                self.display_mode = DisplayMode::from_bits(value & 0x3);
                if E::is_a() {
                    self.vram_block = value >> 2 & 0x3;
                } else { assert_eq!(self.vram_block, 0) }
                self.tile_obj_1d_bound = value >> 4 & 0x3;
                self.flags.bits = self.flags.bits & !0x00FF_0000 | (value as u32) << 16 & DISPCNTFlags::all().bits;
                // TODO: Use trait specialization instead of this
                if !E::is_a() {
                    self.flags.remove(DISPCNTFlags::BITMAP_OBJ_1D);
                }
            },
            3 => {
                self.flags.bits = self.flags.bits & !0xFF00_0000 | (value as u32) << 24 & DISPCNTFlags::all().bits;
                if E::is_a() {
                    self.screen_base = value >> 3 & 0x7;
                    self.char_base = value & 0x7;
                } else {
                    assert_eq!(self.screen_base, 0);
                    assert_eq!(self.char_base, 0);
                }
            },
            _ => unreachable!(),
        }
    }
}

bitflags! {
    pub struct DISPSTATFlags: u16 {
        const VBLANK = 1 << 0;
        const HBLANK = 1 << 1;
        const VCOUNTER = 1 << 2;
        const VBLANK_IRQ_ENABLE = 1 << 3;
        const HBLANK_IRQ_ENABLE = 1 << 4;
        const VCOUNTER_IRQ_ENALBE = 1 << 5;
    }
}

pub struct DISPSTAT {
    pub flags: DISPSTATFlags,
    pub vcount_setting: u16,
}

impl DISPSTAT {
    pub fn new() -> DISPSTAT {
        DISPSTAT {
            flags: DISPSTATFlags::empty(),
            vcount_setting: 0,
        }
    }
}

impl Deref for DISPSTAT {
    type Target = DISPSTATFlags;

    fn deref(&self) -> &DISPSTATFlags {
        &self.flags
    }
}

impl DerefMut for DISPSTAT {
    fn deref_mut(&mut self) -> &mut DISPSTATFlags {
        &mut self.flags
    }
}

impl IORegister for DISPSTAT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.vcount_setting >> 1) as u8 & 0x80 | self.flags.bits as u8,
            1 => self.vcount_setting as u8,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => {
                let old_bits = self.flags.bits;
                self.flags.bits = self.flags.bits & 0x7 | ((value as u16) & !0x7 & DISPSTATFlags::all().bits);
                assert_eq!(old_bits & 0x7, self.flags.bits & 0x7);
                self.vcount_setting = self.vcount_setting & !0x100 | (value as u16 & 0x80) << 8;
            },
            1 => self.vcount_setting = self.vcount_setting & !0xFF | value as u16,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct BGCNT {
    pub priority: u8,
    pub tile_block: u8,
    pub mosaic: bool,
    pub bpp8: bool,
    pub map_block: u8,
    pub wrap: bool,
    pub screen_size: u8,
}

impl BGCNT {
    pub fn new() -> BGCNT {
        BGCNT {
            priority: 0,
            tile_block: 0,
            mosaic: false,
            bpp8: false,
            map_block: 0,
            wrap: false,
            screen_size: 0,
        }
    }
}

impl IORegister for BGCNT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.bpp8 as u8) << 7 | (self.mosaic as u8) << 6 | self.tile_block << 2 | self.priority,
            1 => self.screen_size << 6 | (self.wrap as u8) << 5 | self.map_block,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => {
                self.priority = value & 0x3;
                self.tile_block = value >> 2 & 0xF;
                self.mosaic = value >> 6 & 0x1 != 0;
                self.bpp8 = value >> 7 & 0x1 != 0;
            },
            1 => {
                self.map_block = value & 0x1F;
                self.wrap = value >> 5 & 0x1 != 0;
                self.screen_size = value >> 6 & 0x3;
            },
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct OFS {
    pub offset: u16,
}

impl OFS {
    pub fn new() -> OFS {
        OFS {
            offset: 0,
        }
    }
}

impl IORegister for OFS {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => self.offset as u8,
            1 => (self.offset >> 8) as u8,
            _ => unreachable!(),
        }
    }
    
    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => self.offset = self.offset & !0xFF | value as u16,
            1 => self.offset = self.offset & !0x100 | (value as u16) << 8 & 0x100,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct RotationScalingParameter {
    value: i16,
}

impl RotationScalingParameter {
    pub fn new() -> RotationScalingParameter {
        RotationScalingParameter {
            value: 0,
        }
    }

    pub fn get_float_from_u16(value: u16) -> f64 {
        (value >> 8) as i8 as i32 as f64 + (value >> 0) as u8 as f64 / 256.0
    }
}

impl IORegister for RotationScalingParameter {
    fn read(&self, _byte: usize) -> u8 { return 0 }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        let offset = byte * 8;
        match byte {
            0 | 1 => self.value = ((self.value as u32) & !(0xFF << offset) | (value as u32) << offset) as i16,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct ReferencePointCoord {
    value: i32,
}

impl ReferencePointCoord {
    pub fn new() -> ReferencePointCoord {
        ReferencePointCoord {
            value: 0,
        }
    }

    pub fn integer(&self) -> i32 {
        self.value >> 8
    }
}

impl IORegister for ReferencePointCoord {
    fn read(&self, _byte: usize) -> u8 { 0 }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        let offset = byte * 8;
        match byte {
            0 ..= 2 => self.value = (self.value as u32 & !(0xFF << offset) | (value as u32) << offset) as i32,
            3 => {
                self.value = (self.value as u32 & !(0xFF << offset) | (value as u32 & 0xF) << offset) as i32;
                if self.value & 0x0800_0000 != 0 { self.value = ((self.value as u32) | 0xF000_0000) as i32 }
            },
            _ => unreachable!(),
        }
    }
}

impl std::ops::AddAssign<RotationScalingParameter> for ReferencePointCoord {
    fn add_assign(&mut self, rhs: RotationScalingParameter) {
        *self = Self {
            value: self.value.wrapping_add(rhs.value as i32),
        }
    }
}

#[derive(Clone, Copy)]
pub struct WindowDimensions {
    pub coord2: u8,
    pub coord1: u8,
}

impl WindowDimensions {
    pub fn new() -> WindowDimensions {
        WindowDimensions {
            coord2: 0,
            coord1: 0,
        }
    }
}

impl IORegister for WindowDimensions {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => self.coord2,
            1 => self.coord1,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => self.coord2 = value,
            1 => self.coord1 = value,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct WindowControl {
    pub bg0_enable: bool,
    pub bg1_enable: bool,
    pub bg2_enable: bool,
    pub bg3_enable: bool,
    pub obj_enable: bool,
    pub color_special_enable: bool,
}

impl WindowControl {
    pub fn new() -> WindowControl {
        WindowControl {
            bg0_enable: false,
            bg1_enable: false,
            bg2_enable: false,
            bg3_enable: false,
            obj_enable: false,
            color_special_enable: false,
        }
    }

    pub fn all() -> WindowControl {
        WindowControl {
            bg0_enable: true,
            bg1_enable: true,
            bg2_enable: true,
            bg3_enable: true,
            obj_enable: true,
            color_special_enable: true,
        }
    }
}

impl IORegister for WindowControl {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.color_special_enable as u8) << 5 | (self.obj_enable as u8) << 4 | (self.bg3_enable as u8) << 3 |
                (self.bg2_enable as u8) << 2 | (self.bg1_enable as u8) << 1 | (self.bg0_enable as u8) << 0,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => {
                self.color_special_enable = value >> 5 & 0x1 != 0;
                self.obj_enable = value >> 4 & 0x1 != 0;
                self.bg3_enable = value >> 3 & 0x1 != 0;
                self.bg2_enable = value >> 2 & 0x1 != 0;
                self.bg1_enable = value >> 1 & 0x1 != 0;
                self.bg0_enable = value >> 0 & 0x1 != 0;
            },
            _ => unreachable!(),
        }
    }
}

pub struct MosaicSize {
    pub h_size: u16,
    pub v_size: u16,
}

impl MosaicSize {
    pub fn new() -> MosaicSize {
        MosaicSize {
            h_size: 1,
            v_size: 1,
        }
    }

    pub fn read(&self) -> u8 {
        (self.v_size as u8 - 1) << 4 | (self.h_size as u8 - 1)
    }

    pub fn write(&mut self, value: u8) {
        self.h_size = (value as u16 & 0xF) + 1;
        self.v_size = (value as u16 >> 4) + 1;
    }
}

pub struct MOSAIC {
    pub bg_size: MosaicSize,
    pub obj_size: MosaicSize,
}

impl MOSAIC {
    pub fn new() -> MOSAIC {
        MOSAIC {
            bg_size: MosaicSize::new(),
            obj_size: MosaicSize::new(),
        }
    }
}

impl IORegister for MOSAIC {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => self.bg_size.read(),
            1 => self.obj_size.read(),
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => self.bg_size.write(value),
            1 => self.obj_size.write(value),
            _ => unreachable!(),
        }
    }
}

pub struct BLDCNTTargetPixelSelection {
    pub enabled: [bool; 6]
}

impl BLDCNTTargetPixelSelection {
    pub fn new() -> BLDCNTTargetPixelSelection {
        BLDCNTTargetPixelSelection {
            enabled: [false; 6],
        }
    }

    pub fn read(&self) -> u8 {
        (self.enabled[0] as u8) << 0 |
        (self.enabled[1] as u8) << 1 |
        (self.enabled[2] as u8) << 2 |
        (self.enabled[3] as u8) << 3 |
        (self.enabled[4] as u8) << 4 |
        (self.enabled[5] as u8) << 5
    }

    pub fn write(&mut self, value: u8) {
        self.enabled[0] = value >> 0 & 0x1 != 0;
        self.enabled[1] = value >> 1 & 0x1 != 0;
        self.enabled[2] = value >> 2 & 0x1 != 0;
        self.enabled[3] = value >> 3 & 0x1 != 0;
        self.enabled[4] = value >> 4 & 0x1 != 0;
        self.enabled[5] = value >> 5 & 0x1 != 0;
    }
}

#[derive(Clone, Copy)]
pub enum ColorSFX {
    None = 0,
    AlphaBlend = 1,
    BrightnessInc = 2,
    BrightnessDec = 3,
}

impl ColorSFX {
    pub fn from(value: u8) -> ColorSFX {
        use ColorSFX::*;
        match value {
            0 => None,
            1 => AlphaBlend,
            2 => BrightnessInc,
            3 => BrightnessDec,
            _ => unreachable!(),
        }
    }
}

pub struct BLDCNT {
    pub target_pixel1: BLDCNTTargetPixelSelection,
    pub effect: ColorSFX,
    pub target_pixel2: BLDCNTTargetPixelSelection,
}

impl BLDCNT {
    pub fn new() -> BLDCNT {
        BLDCNT {
            target_pixel1: BLDCNTTargetPixelSelection::new(),
            effect: ColorSFX::None,
            target_pixel2: BLDCNTTargetPixelSelection::new(),
        }
    }
}

impl IORegister for BLDCNT {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => (self.effect as u8) << 6 | self.target_pixel1.read(),
            1 => self.target_pixel2.read(),
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => {
                self.target_pixel1.write(value);
                self.effect = ColorSFX::from(value >> 6);
            },
            1 => self.target_pixel2.write(value),
            _ => unreachable!(),
        }
    }
}

pub struct BLDALPHA {
    raw_eva: u8,
    raw_evb: u8,
    pub eva: u16,
    pub evb: u16,
}

impl BLDALPHA {
    pub fn new() -> BLDALPHA {
        BLDALPHA {
            raw_eva: 0,
            raw_evb: 0,
            eva: 0,
            evb: 0,
        }
    }
}

impl IORegister for BLDALPHA {
    fn read(&self, byte: usize) -> u8 {
        match byte {
            0 => self.raw_eva,
            1 => self.raw_evb,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => {
                self.raw_eva = value & 0x1F;
                self.eva = std::cmp::min(0x10, self.raw_eva as u16);
            }
            1 => {
                self.raw_evb = value & 0x1F;
                self.evb = std::cmp::min(0x10, self.raw_evb as u16);
            }
            _ => unreachable!(),
        }
    }
}

pub struct BLDY {
    pub evy: u8,
}

impl BLDY {
    pub fn new() -> BLDY {
        BLDY {
            evy: 0,
        }
    }
}

impl IORegister for BLDY {
    fn read(&self, _byte: usize) -> u8 { 0 }

    fn write(&mut self, __scheduler: &mut Scheduler, byte: usize, value: u8) {
        match byte {
            0 => self.evy = std::cmp::min(0x10, value & 0x1F),
            1 => (),
            _ => unreachable!(),
        }
    }
}

bitflags! {
    pub struct POWCNT1: u32 {
        const ENABLE_LCDS = 1 << 0;
        const ENABLE_ENGINE_A = 1 << 1;
        const ENABLE_3D_RENDERING = 1 << 2;
        const ENABLE_3D_GEOMETRY = 1 << 3;
        const ENABLE_ENGINE_B = 1 << 9;
        const SWAP_DISPLAY = 1 << 15;
    }
}

impl IORegister for POWCNT1 {
    fn read(&self, byte: usize) -> u8 {
        assert!(byte < 4);
        (self.bits >> (byte * 4)) as u8
    }

    fn write(&mut self, _scheduler: &mut Scheduler, byte: usize, value: u8) {
        assert!(byte < 4);
        let mask = 0xFF << byte * 4;
        self.bits = (self.bits & !mask) | (value as u32) << byte * 4;
        assert!(self.contains(POWCNT1::ENABLE_LCDS)) // TODO: Figure out what this does
    }
    
}
