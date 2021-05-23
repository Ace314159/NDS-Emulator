extern crate glfw;
extern crate imgui_opengl_renderer;

use glfw::{Action, Context, Glfw, Window};

use std::collections::HashSet;
use std::{path::PathBuf, time::Instant};

use nds_core::nds::{self, NDS};

pub struct Display {
    window: Window,
    events: std::sync::mpsc::Receiver<(f64, glfw::WindowEvent)>,
    screen_tex: u32,

    imgui_renderer: imgui_opengl_renderer::Renderer,
    glfw: Glfw, // Dropped last

    prev_frame_time: Instant,
    prev_fps_update_time: Instant,
    frames_passed: u32,
}

impl Display {
    const WIDTH: usize = nds::WIDTH;
    const HEIGHT: usize = 2 * nds::HEIGHT;
    const SCALE: usize = 1;

    pub fn new(imgui: &mut imgui::Context) -> Display {
        let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
        glfw.set_error_callback(glfw::FAIL_ON_ERRORS);

        let width = (Display::WIDTH * Display::SCALE) as u32;
        let height = 19 + (Display::HEIGHT * Display::SCALE) as u32; // TODO: Don't hardcode main menu bar height
        let (mut window, events) = glfw
            .create_window(width, height, "NDS Emulator", glfw::WindowMode::Windowed)
            .expect("Failed to create GLFW window!");
        window.make_current();
        window.set_all_polling(true);
        gl::load_with(|name| window.get_proc_address(name));

        let imgui_renderer =
            imgui_opengl_renderer::Renderer::new(imgui, |s| window.get_proc_address(s) as _);
        imgui.set_ini_filename(None);
        Self::init_imgui(&window, imgui.io_mut());
        imgui.set_platform_name(Some(imgui::ImString::from(format!(
            "imgui-glfw {}",
            env!("CARGO_PKG_VERSION")
        ))));

        let mut screen_tex = 0u32;
        let mut fbo = 0u32;
        let color_black = [1f32, 0f32, 0f32];
        unsafe {
            gl::Enable(gl::DEBUG_OUTPUT);
            gl::Enable(gl::DEBUG_OUTPUT_SYNCHRONOUS);
            gl::DebugMessageCallback(Some(gl_debug_callback), std::ptr::null_mut());

            gl::GenTextures(1, &mut screen_tex as *mut u32);
            gl::BindTexture(gl::TEXTURE_2D, screen_tex);
            gl::TexParameterfv(
                gl::TEXTURE_2D,
                gl::TEXTURE_BORDER_COLOR,
                &color_black as *const f32,
            );
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
            gl::TexStorage2D(
                gl::TEXTURE_2D,
                1,
                gl::RGBA8,
                Display::WIDTH as i32,
                Display::HEIGHT as i32,
            );

            gl::GenFramebuffers(1, &mut fbo as *mut u32);
            gl::BindFramebuffer(gl::READ_FRAMEBUFFER, fbo);
            gl::FramebufferTexture2D(
                gl::READ_FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                screen_tex,
                0,
            );
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
        }

        Display {
            glfw,
            window,
            events,
            screen_tex,

            imgui_renderer,

            prev_frame_time: Instant::now(),
            prev_fps_update_time: Instant::now(),
            frames_passed: 0,
        }
    }

    fn init_imgui(window: &Window, io: &mut imgui::Io) {
        use imgui::Key;
        let content_scale = window.get_content_scale();
        io.display_framebuffer_scale = [content_scale.0, content_scale.1];
        let window_size = window.get_size();
        io.display_size = [window_size.0 as f32, window_size.1 as f32];
        io.backend_flags
            .insert(imgui::BackendFlags::HAS_MOUSE_CURSORS);
        io.backend_flags
            .insert(imgui::BackendFlags::HAS_SET_MOUSE_POS);
        io[Key::Tab] = glfw::Key::Tab as _;
        io[Key::LeftArrow] = glfw::Key::Left as _;
        io[Key::RightArrow] = glfw::Key::Right as _;
        io[Key::UpArrow] = glfw::Key::Up as _;
        io[Key::DownArrow] = glfw::Key::Down as _;
        io[Key::PageUp] = glfw::Key::PageUp as _;
        io[Key::PageDown] = glfw::Key::PageDown as _;
        io[Key::Home] = glfw::Key::Home as _;
        io[Key::End] = glfw::Key::End as _;
        io[Key::Insert] = glfw::Key::Insert as _;
        io[Key::Delete] = glfw::Key::Delete as _;
        io[Key::Backspace] = glfw::Key::Backspace as _;
        io[Key::Space] = glfw::Key::Space as _;
        io[Key::Enter] = glfw::Key::Enter as _;
        io[Key::Escape] = glfw::Key::Escape as _;
        io[Key::KeyPadEnter] = glfw::Key::KpEnter as _;
        io[Key::A] = glfw::Key::A as _;
        io[Key::C] = glfw::Key::C as _;
        io[Key::V] = glfw::Key::V as _;
        io[Key::X] = glfw::Key::X as _;
        io[Key::Y] = glfw::Key::Y as _;
        io[Key::Z] = glfw::Key::Z as _;
    }

    pub fn run_main_loop<F: FnMut(&mut Display)>(&mut self, main_loop: F) {
        let mut main_loop = main_loop;
        while !self.window.should_close() {
            main_loop(self);
        }
    }

    fn prepare_frame(&mut self, io: &mut imgui::Io) {
        if io.want_set_mouse_pos {
            self.window
                .set_cursor_pos(io.mouse_pos[0] as f64, io.mouse_pos[1] as f64);
        }
        let (window_width, window_height) = self.window.get_size();
        io.display_size = [window_width as f32, window_height as f32];
        let (display_width, display_height) = self.window.get_framebuffer_size();
        if display_width > 0 && display_height > 0 {
            io.display_framebuffer_scale = [
                display_width as f32 / window_width as f32,
                display_height as f32 / window_height as f32,
            ];
        }
    }

    fn prepare_render(&mut self, ui: &imgui::Ui) {
        use glfw::StandardCursor::*;
        let io = ui.io();
        if io
            .config_flags
            .contains(imgui::ConfigFlags::NO_MOUSE_CURSOR_CHANGE)
        {
            return;
        }
        let mouse_cursor = ui.mouse_cursor();
        match mouse_cursor {
            Some(mouse_cursor) if !io.mouse_draw_cursor => {
                self.window.set_cursor_mode(glfw::CursorMode::Normal);
                self.window
                    .set_cursor(Some(glfw::Cursor::standard(match mouse_cursor {
                        imgui::MouseCursor::Arrow => Arrow,
                        imgui::MouseCursor::TextInput => IBeam,
                        imgui::MouseCursor::ResizeAll => Arrow, // TODO: Fix when updating GLFW
                        imgui::MouseCursor::ResizeNS => VResize,
                        imgui::MouseCursor::ResizeEW => HResize,
                        imgui::MouseCursor::ResizeNESW => Arrow, // TODO: Fix when updating GLFW
                        imgui::MouseCursor::ResizeNWSE => Arrow, // TODO: Fix when updating GLFW
                        imgui::MouseCursor::Hand => Hand,
                        imgui::MouseCursor::NotAllowed => Arrow, // TODO: Fix when updating GLFW
                    })));
            }
            _ => self.window.set_cursor_mode(glfw::CursorMode::Hidden),
        }
    }

    pub fn render_main(
        &mut self,
        nds: &mut NDS,
        imgui: &mut imgui::Context,
        main_menu_height: f32,
    ) -> (HashSet<glfw::Key>, Vec<PathBuf>) {
        let screens = nds.get_screens();
        let (width, height) = self.window.get_size();
        let height = height - main_menu_height as i32;

        let (tex_x, tex_y) = if width * Display::HEIGHT as i32 > height * Display::WIDTH as i32 {
            let scaled_width =
                (Display::WIDTH as f32 / Display::HEIGHT as f32 * height as f32) as i32;
            ((width - scaled_width) / 2, 0)
        } else if width * (Display::HEIGHT as i32) < height * Display::WIDTH as i32 {
            let scaled_height =
                (Display::HEIGHT as f32 / Display::WIDTH as f32 * width as f32) as i32;
            (0, (height - scaled_height) / 2)
        } else {
            (0, 0)
        };

        let x_start = tex_x;
        let y_start = tex_y;
        let x_end = width - tex_x;
        let y_end = height - tex_y;

        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.screen_tex);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                nds::WIDTH as i32,
                nds::HEIGHT as i32,
                gl::RGBA,
                gl::UNSIGNED_SHORT_1_5_5_5_REV,
                screens[0].as_ptr() as *const std::ffi::c_void,
            );
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                nds::HEIGHT as i32,
                nds::WIDTH as i32,
                nds::HEIGHT as i32,
                gl::RGBA,
                gl::UNSIGNED_SHORT_1_5_5_5_REV,
                screens[1].as_ptr() as *const std::ffi::c_void,
            );
            // Flip src0 and src1 because OpenGL wants the texture flipped vertically
            gl::BlitFramebuffer(
                0,
                Display::HEIGHT as i32,
                Display::WIDTH as i32,
                0,
                x_start,
                y_start,
                x_end,
                y_end,
                gl::COLOR_BUFFER_BIT,
                gl::NEAREST,
            );
        }

        let io = imgui.io_mut();

        self.glfw.poll_events();

        let mut keys_pressed = HashSet::new();
        let mut modifiers = HashSet::new();
        let mut files_dropped = Vec::new();
        let old_mouse_pressed =
            self.window.get_mouse_button(glfw::MouseButtonLeft) == Action::Press;
        for (_, event) in glfw::flush_messages(&self.events) {
            Display::handle_event(io, &event);
            match event {
                glfw::WindowEvent::Key(key, _, action, new_modifiers)
                    if !io.want_capture_keyboard =>
                {
                    if action != Action::Release {
                        keys_pressed.insert(key);
                        modifiers.insert(new_modifiers);
                    }
                    let nds_key = match key {
                        glfw::Key::A => nds::Key::A,
                        glfw::Key::B => nds::Key::B,
                        glfw::Key::X => nds::Key::X,
                        glfw::Key::Y => nds::Key::Y,
                        glfw::Key::E => nds::Key::Select,
                        glfw::Key::T => nds::Key::Start,
                        glfw::Key::Right => nds::Key::Right,
                        glfw::Key::Left => nds::Key::Left,
                        glfw::Key::Up => nds::Key::Up,
                        glfw::Key::Down => nds::Key::Down,
                        glfw::Key::R => nds::Key::R,
                        glfw::Key::L => nds::Key::L,
                        _ => continue,
                    };
                    match action {
                        Action::Press => nds.press_key(nds_key),
                        Action::Release => nds.release_key(nds_key),
                        _ => continue,
                    };
                }
                glfw::WindowEvent::MouseButton(glfw::MouseButtonLeft, Action::Press, _)
                | glfw::WindowEvent::MouseButton(glfw::MouseButtonLeft, Action::Release, _)
                    if !io.want_capture_mouse =>
                {
                    self.check_stylus(
                        nds,
                        main_menu_height as f64,
                        x_start,
                        y_start,
                        x_end - x_start,
                        y_end - y_start,
                    )
                }
                glfw::WindowEvent::CursorPos(_, _)
                    if old_mouse_pressed && !io.want_capture_mouse =>
                {
                    self.check_stylus(
                        nds,
                        main_menu_height as f64,
                        x_start,
                        y_start,
                        x_end - x_start,
                        y_end - y_start,
                    )
                }
                glfw::WindowEvent::FileDrop(paths) => files_dropped = paths,
                _ => (),
            }
        }

        (keys_pressed, files_dropped)
    }

    pub fn render_imgui<F>(
        &mut self,
        imgui: &mut imgui::Context,
        keys_pressed: HashSet<glfw::Key>,
        imgui_draw: F,
    ) where
        F: FnOnce(&imgui::Ui, HashSet<glfw::Key>),
    {
        let io = imgui.io_mut();
        self.prepare_frame(io);
        io.update_delta_time(Instant::now() - self.prev_frame_time);
        let ui = imgui.frame();
        imgui_draw(&ui, keys_pressed);
        self.prepare_render(&ui);
        self.imgui_renderer.render(ui);

        // while Instant::now().duration_since(self.prev_frame_time) < Display::FRAME_PERIOD {}
        self.window.swap_buffers();
        self.prev_frame_time = Instant::now();
        self.frames_passed += 1;

        let time_passed = self.prev_fps_update_time.elapsed().as_secs_f64();
        if time_passed >= 1.0 {
            let fps = self.frames_passed as f64 / time_passed;
            self.window
                .set_title(&format!("NDS Emulator - {:.2} FPS", fps));
            self.frames_passed = 0;
            self.prev_fps_update_time = Instant::now();
        }
    }

    fn handle_event(io: &mut imgui::Io, event: &glfw::WindowEvent) {
        use glfw::{Modifiers, MouseButton, WindowEvent::*};
        match *event {
            MouseButton(button, action, _modifiers) => {
                let index = match button {
                    MouseButton::Button1 => 0,
                    MouseButton::Button2 => 1,
                    MouseButton::Button3 => 2,
                    MouseButton::Button4 => 3,
                    MouseButton::Button5 => 4,
                    _ => return,
                };
                io.mouse_down[index] = action != Action::Release;
            }
            CursorPos(x, y) => io.mouse_pos = [x as f32, y as f32],
            Scroll(x_offset, y_offset) => {
                io.mouse_wheel_h += x_offset as f32;
                io.mouse_wheel += y_offset as f32;
            }
            Key(key, _scancode, action, modifiers) => {
                if (key as usize) < io.keys_down.len() {
                    io.keys_down[key as usize] = action != Action::Release
                }
                io.key_shift = modifiers.contains(Modifiers::Shift);
                io.key_ctrl = modifiers.contains(Modifiers::Control);
                io.key_alt = modifiers.contains(Modifiers::Alt);
                io.key_super = modifiers.contains(Modifiers::Super);
            }
            Char(char) => io.add_input_character(char),
            _ => (),
        }
    }

    fn check_stylus(
        &self,
        nds: &mut NDS,
        main_menu_height: f64,
        tex_x: i32,
        tex_y: i32,
        tex_width: i32,
        tex_height: i32,
    ) {
        let pressed = self.window.get_mouse_button(glfw::MouseButtonLeft) == Action::Press;
        if !pressed {
            nds.release_screen();
            return;
        }
        let (cursor_x, cursor_y) = self.window.get_cursor_pos();
        let cursor_y = cursor_y - main_menu_height;

        let (width_factor, height_factor) = (
            tex_width as f64 / Display::WIDTH as f64,
            tex_height as f64 / Display::HEIGHT as f64,
        );
        let clamp = |val, max| {
            if val < 0.0 {
                0.0
            } else if val > max as f64 {
                max as f64
            } else {
                val
            }
        };

        let touch_x = clamp((cursor_x - tex_x as f64) / width_factor, nds::WIDTH);
        let touch_y = clamp(
            (cursor_y - tex_y as f64 - (tex_height / 2) as f64) / height_factor,
            nds::HEIGHT,
        );
        nds.press_screen(touch_x as usize, touch_y as usize);
    }
}

extern "system" fn gl_debug_callback(
    _source: u32,
    _type: u32,
    _id: u32,
    sev: u32,
    _len: i32,
    message: *const i8,
    _param: *mut std::ffi::c_void,
) {
    if sev == gl::DEBUG_SEVERITY_NOTIFICATION {
        return;
    }

    unsafe {
        let message = std::ffi::CStr::from_ptr(message).to_str().unwrap();
        panic!("OpenGL Debug message: {}", message);
    }
}
