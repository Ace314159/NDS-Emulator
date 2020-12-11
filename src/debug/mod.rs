mod windows;

use std::collections::HashSet;

use imgui::*;
use glfw::Key;

pub use windows::*;
use super::{Engine, GraphicsType, NDS};

pub struct DebugWindow<S> where S: DebugWindowState {
    title: ImString,
    texture: Texture,
    opened: bool,
    scale: f32,
    state: S,
}

impl<S> DebugWindow<S> where S: DebugWindowState {
    const SCALE_OFFSET: f32 = 0.1;

    pub fn new(title: &str) -> DebugWindow<S> {
        DebugWindow {
            title: ImString::new(title),
            texture: Texture::new(),
            opened: false,
            scale: 1.0,
            state: S::new(),
        }
    }

    pub fn menu_item(&mut self, ui: &Ui) {
        let clicked = MenuItem::new(&self.title).selected(self.opened).build(ui);
        if clicked { self.opened = !self.opened }
    }

    pub fn render(&mut self, nds: &mut NDS, ui: &Ui, keys_pressed: &HashSet<Key>) {
        if !self.opened { return }

        let (pixels, width, height) = self.state.get_pixels(nds);

        self.texture.update_pixels(pixels, width, height);
        let title = self.title.clone();
        let mut opened = self.opened;
        Window::new(&title)
        .always_auto_resize(true)
        .opened(&mut opened)
        .build(ui, || {
            if ui.is_window_focused() {
            if keys_pressed.contains(&Key::Equal) { self.scale += Self::SCALE_OFFSET }
            if keys_pressed.contains(&Key::Minus) { self.scale -= Self::SCALE_OFFSET }
            }
            self.state.render(ui);
            self.texture.render(self.scale).build(ui);
        });
        self.opened = opened;
    }
}

pub trait DebugWindowState {
    fn new() -> Self;
    fn render(&mut self, ui: &Ui);
    fn get_pixels(&self, nds: &mut NDS) -> (Vec<u16>, usize, usize);
    
    const ENGINES: [Engine; 2] = [Engine::A, Engine::B];
    const GRAPHICS_TYPES: [GraphicsType; 2] = [GraphicsType::BG, GraphicsType::OBJ];
}

struct Texture {
    tex: u32,
    width: f32,
    height: f32,
}

impl Texture {
    pub fn new() -> Texture {
        Texture {
            tex: 0,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn update_pixels(&mut self, pixels: Vec<u16>, width: usize, height: usize) {
        let width = width as f32;
        let height = height as f32;
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            unsafe {
                gl::DeleteTextures(1, &mut self.tex as *mut u32);
                gl::GenTextures(1, &mut self.tex as *mut u32);
                gl::BindTexture(gl::TEXTURE_2D, self.tex);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
                gl::TexStorage2D(gl::TEXTURE_2D, 1, gl::RGBA8, self.width as i32, self.height as i32);
            }
        }
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.tex);
            gl::TexSubImage2D(gl::TEXTURE_2D, 0, 0, 0, self.width as i32, self.height as i32,
                gl::RGBA, gl::UNSIGNED_SHORT_1_5_5_5_REV, pixels.as_ptr() as *const std::ffi::c_void);
        }
    }

    pub fn render(&self, scale: f32) -> Image {
        Image::new(TextureId::from(self.tex as usize), [self.width * scale, self.height * scale])
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe { gl::DeleteTextures(1, &mut self.tex as *mut u32) }
    }
}
