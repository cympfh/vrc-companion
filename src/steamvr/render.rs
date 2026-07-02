//! SteamVR オーバーレイをオフスクリーンGLで描画するための WGL コンテキスト + egui_glow 統合。
//!
//! `eframe` がプロセス内で唯一の `winit::EventLoop` を保持しているため winit/glutin は使えない
//! (2つ目の EventLoop 生成はプロセス全体のフラグでエラーになる)。代わりに `windows-sys` で
//! 隠しウィンドウ + WGL コンテキストを直接作る。WGL コンテキストはスレッドアフィニティを
//! 持つため、この構造体の生成・`render`・破棄は全て同じスレッド上で行うこと
//! (session.rs では専用の `std::thread::spawn` スレッド上でのみ扱う)。

use std::ffi::{CString, c_void};
use std::sync::Arc;

use glow::HasContext;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::{GetDC, HDC, ReleaseDC};
use windows_sys::Win32::Graphics::OpenGL::{
    ChoosePixelFormat, HGLRC, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_SUPPORT_OPENGL,
    PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR, SetPixelFormat, wglCreateContext, wglDeleteContext,
    wglGetProcAddress, wglMakeCurrent,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_OWNDC, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW,
    WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

use super::bridge::{OverlayAction, OverlaySnapshot, overlay_fields};

const WINDOW_CLASS_NAME: &str = "VrcCompanionOverlayGlWindow";

/// SteamVR オーバーレイに表示するegui UIをオフスクリーンで描画し、GLテクスチャとして保持する。
pub struct GlOverlayRenderer {
    hwnd: HWND,
    hdc: HDC,
    hglrc: HGLRC,
    gl: Arc<glow::Context>,
    egui_ctx: egui::Context,
    painter: egui_glow::Painter,
    framebuffer: glow::Framebuffer,
    texture: glow::Texture,
    width: u32,
    height: u32,
}

impl GlOverlayRenderer {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        unsafe {
            let hwnd = create_hidden_window()?;
            let hdc = GetDC(hwnd);
            if hdc.is_null() {
                DestroyWindow(hwnd);
                return Err("GetDC が失敗した".to_string());
            }

            if let Err(e) = setup_pixel_format(hdc) {
                ReleaseDC(hwnd, hdc);
                DestroyWindow(hwnd);
                return Err(e);
            }

            let hglrc = wglCreateContext(hdc);
            if hglrc.is_null() {
                ReleaseDC(hwnd, hdc);
                DestroyWindow(hwnd);
                return Err("wglCreateContext が失敗した".to_string());
            }
            if wglMakeCurrent(hdc, hglrc) == 0 {
                wglDeleteContext(hglrc);
                ReleaseDC(hwnd, hdc);
                DestroyWindow(hwnd);
                return Err("wglMakeCurrent が失敗した".to_string());
            }

            let gl = Arc::new(glow::Context::from_loader_function(load_gl_function));

            match Self::init_gl_resources(&gl, width, height) {
                Ok((painter, framebuffer, texture)) => {
                    let egui_ctx = egui::Context::default();
                    setup_fonts(&egui_ctx);
                    Ok(Self {
                        hwnd,
                        hdc,
                        hglrc,
                        gl,
                        egui_ctx,
                        painter,
                        framebuffer,
                        texture,
                        width,
                        height,
                    })
                }
                Err(e) => {
                    wglMakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
                    wglDeleteContext(hglrc);
                    ReleaseDC(hwnd, hdc);
                    DestroyWindow(hwnd);
                    Err(e)
                }
            }
        }
    }

    fn init_gl_resources(
        gl: &Arc<glow::Context>,
        width: u32,
        height: u32,
    ) -> Result<(egui_glow::Painter, glow::Framebuffer, glow::Texture), String> {
        unsafe {
            let texture = gl
                .create_texture()
                .map_err(|e| format!("create_texture 失敗: {}", e))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                None,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );

            let framebuffer = gl
                .create_framebuffer()
                .map_err(|e| format!("create_framebuffer 失敗: {}", e))?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(texture),
                0,
            );

            let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            if status != glow::FRAMEBUFFER_COMPLETE {
                return Err(format!("FBO が未完成: status=0x{:x}", status));
            }

            let painter = egui_glow::Painter::new(gl.clone(), "", None, false)
                .map_err(|e| format!("egui_glow::Painter::new 失敗: {}", e))?;

            Ok((painter, framebuffer, texture))
        }
    }

    /// 現在の `snapshot` を反映したUIを描画し、結果のテクスチャと今フレームでクリックされた
    /// `OverlayAction` 一覧を返す。`events` は SteamVR から拾ったポインタ入力(session.rs)。
    pub fn render(
        &mut self,
        snapshot: &OverlaySnapshot,
        events: Vec<egui::Event>,
    ) -> Result<(glow::NativeTexture, Vec<OverlayAction>), String> {
        unsafe {
            if wglMakeCurrent(self.hdc, self.hglrc) == 0 {
                return Err("wglMakeCurrent が失敗した".to_string());
            }
            self.gl
                .bind_framebuffer(glow::FRAMEBUFFER, Some(self.framebuffer));
        }

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(self.width as f32, self.height as f32),
            )),
            events,
            ..Default::default()
        };

        let mut clicked: Vec<OverlayAction> = Vec::new();

        let output = self.egui_ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for field in overlay_fields(snapshot) {
                    let mut checked = field.enabled;
                    let resp = if field.indent {
                        ui.indent(field.label, |ui| ui.checkbox(&mut checked, field.label))
                            .inner
                    } else {
                        ui.checkbox(&mut checked, field.label)
                    };
                    if resp.clicked() {
                        clicked.push(field.action);
                    }
                }
                if ui.button("📝 call QvPen").clicked() {
                    clicked.push(OverlayAction::CallQvPen);
                }
            });
        });

        let clipped_primitives = self
            .egui_ctx
            .tessellate(output.shapes, output.pixels_per_point);

        self.painter
            .clear([self.width, self.height], [0.0, 0.0, 0.0, 0.0]);
        self.painter.paint_and_update_textures(
            [self.width, self.height],
            output.pixels_per_point,
            &clipped_primitives,
            &output.textures_delta,
        );

        unsafe {
            self.gl.flush();
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        }

        Ok((self.texture, clicked))
    }
}

impl Drop for GlOverlayRenderer {
    fn drop(&mut self) {
        unsafe {
            wglMakeCurrent(self.hdc, self.hglrc);
            self.gl.delete_texture(self.texture);
            self.gl.delete_framebuffer(self.framebuffer);
        }
        self.painter.destroy();
        unsafe {
            wglMakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
            wglDeleteContext(self.hglrc);
            ReleaseDC(self.hwnd, self.hdc);
            DestroyWindow(self.hwnd);
        }
    }
}

unsafe fn setup_pixel_format(hdc: HDC) -> Result<(), String> {
    unsafe {
        let mut pfd: PIXELFORMATDESCRIPTOR = std::mem::zeroed();
        pfd.nSize = std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16;
        pfd.nVersion = 1;
        pfd.dwFlags = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
        pfd.iPixelType = PFD_TYPE_RGBA;
        pfd.cColorBits = 32;
        pfd.cDepthBits = 24;
        pfd.cStencilBits = 8;

        let format = ChoosePixelFormat(hdc, &pfd);
        if format == 0 {
            return Err("ChoosePixelFormat が失敗した".to_string());
        }
        if SetPixelFormat(hdc, format, &pfd) == 0 {
            return Err("SetPixelFormat が失敗した".to_string());
        }
        Ok(())
    }
}

unsafe fn create_hidden_window() -> Result<HWND, String> {
    unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        let class_name: Vec<u16> = format!("{}\0", WINDOW_CLASS_NAME).encode_utf16().collect();

        let wnd_class = WNDCLASSW {
            style: CS_OWNDC,
            lpfnWndProc: Some(DefWindowProcW),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };

        if RegisterClassW(&wnd_class) == 0 {
            return Err("RegisterClassW が失敗した".to_string());
        }

        let window_name: Vec<u16> = "VRC Companion Overlay GL\0".encode_utf16().collect();
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_name.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            400,
            300,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        );

        if hwnd.is_null() {
            return Err("CreateWindowExW が失敗した".to_string());
        }

        Ok(hwnd)
    }
}

/// `wglGetProcAddress` を優先し、null の場合は `opengl32.dll` の `GetProcAddress` に
/// フォールバックする(GL1.1コア関数は `wglGetProcAddress` が確実に返さないドライバがある)。
/// main.rs (デスクトップGUI) と同じ日本語フォントを読み込む。デフォルトのegui fontsは
/// CJKグリフを含まないため、これをしないと日本語ラベルが豆腐(□)になる。
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "japanese".to_owned(),
        egui::FontData::from_static(include_bytes!("../../fonts/NotoSansJP-Regular.ttf")),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "japanese".to_owned());
    ctx.set_fonts(fonts);
}

fn load_gl_function(name: &str) -> *const c_void {
    unsafe {
        let cname = match CString::new(name) {
            Ok(c) => c,
            Err(_) => return std::ptr::null(),
        };
        if let Some(addr) = wglGetProcAddress(cname.as_ptr() as *const u8) {
            return addr as *const c_void;
        }
        let module_name: Vec<u16> = "opengl32.dll\0".encode_utf16().collect();
        let module = GetModuleHandleW(module_name.as_ptr());
        if module.is_null() {
            return std::ptr::null();
        }
        match GetProcAddress(module, cname.as_ptr() as *const u8) {
            Some(addr) => addr as *const c_void,
            None => std::ptr::null(),
        }
    }
}
