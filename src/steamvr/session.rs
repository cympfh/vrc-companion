//! OpenVR ダッシュボードオーバーレイのライフサイクル管理。Windows専用 (`#[cfg(windows)]` は
//! 呼び出し元の bridge.rs 側でゲートしている)。
//!
//! `render::GlOverlayRenderer` が毎tick描画したegui UIのGLテクスチャを `SetOverlayTexture`
//! でSteamVRコンポジタに渡す。WGLコンテキストはスレッドアフィニティを持つため、
//! renderer の生成・描画・破棄は全てこの `run` 関数を実行するスレッド上で行う。

use std::ffi::{CString, c_char, c_void};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use super::bridge::{OverlayAction, OverlayHandle, OverlaySnapshot};
use super::ffi::{self, OpenVrLibrary, Texture_t, VrOverlayHandle};
use super::render::GlOverlayRenderer;

const OVERLAY_KEY: &str = "cympfh.vrc_companion";
const OVERLAY_NAME: &str = "VRC Companion";
const OVERLAY_WIDTH: u32 = 400;
const OVERLAY_HEIGHT: u32 = 300;

pub fn start(initial: OverlaySnapshot) -> Option<OverlayHandle> {
    let (snapshot_tx, snapshot_rx) = mpsc::channel::<OverlaySnapshot>();
    let (action_tx, action_rx) = mpsc::channel::<OverlayAction>();

    std::thread::spawn(move || run(initial, snapshot_rx, action_tx));

    Some(OverlayHandle {
        snapshot_tx,
        action_rx,
    })
}

fn run(
    initial: OverlaySnapshot,
    snapshot_rx: Receiver<OverlaySnapshot>,
    _action_tx: Sender<OverlayAction>,
) {
    let lib = match OpenVrLibrary::load() {
        Ok(lib) => lib,
        Err(e) => {
            eprintln!(
                "[SteamVR] 初期化失敗 (SteamVR未起動、またはopenvr_api.dllが見つからない): {}",
                e
            );
            return;
        }
    };

    let overlay_handle = match create_dashboard_overlay(&lib) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("[SteamVR] CreateDashboardOverlay 失敗: {}", e);
            return;
        }
    };

    if let Err(e) = show_overlay(&lib, overlay_handle) {
        eprintln!("[SteamVR] ShowOverlay 失敗: {}", e);
    }

    let mut renderer = match GlOverlayRenderer::new(OVERLAY_WIDTH, OVERLAY_HEIGHT) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[SteamVR] GlOverlayRenderer 初期化失敗: {}", e);
            return;
        }
    };

    let mut latest = initial;
    loop {
        match snapshot_rx.recv_timeout(Duration::from_millis(33)) {
            Ok(s) => latest = s,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        match renderer.render(&latest) {
            Ok(texture) => {
                let mut tex = Texture_t {
                    handle: texture.0.get() as usize as *mut c_void,
                    e_type: ffi::TEXTURE_TYPE_OPENGL,
                    e_color_space: ffi::COLOR_SPACE_AUTO,
                };
                if let Err(e) = set_overlay_texture(&lib, overlay_handle, &mut tex) {
                    eprintln!("[SteamVR] SetOverlayTexture 失敗: {}", e);
                }
            }
            Err(e) => eprintln!("[SteamVR] render 失敗: {}", e),
        }
    }
}

fn create_dashboard_overlay(lib: &OpenVrLibrary) -> Result<VrOverlayHandle, String> {
    let key = CString::new(OVERLAY_KEY).unwrap();
    let name = CString::new(OVERLAY_NAME).unwrap();
    let mut main_handle: VrOverlayHandle = 0;
    let mut thumbnail_handle: VrOverlayHandle = 0;

    let err = unsafe {
        (lib.overlay().create_dashboard_overlay)(
            key.as_ptr() as *mut c_char,
            name.as_ptr() as *mut c_char,
            &mut main_handle,
            &mut thumbnail_handle,
        )
    };
    if err != 0 {
        return Err(format!("EVROverlayError={}", err));
    }
    Ok(main_handle)
}

fn show_overlay(lib: &OpenVrLibrary, handle: VrOverlayHandle) -> Result<(), String> {
    let err = unsafe { (lib.overlay().show_overlay)(handle) };
    if err != 0 {
        return Err(format!("EVROverlayError={}", err));
    }
    Ok(())
}

fn set_overlay_texture(
    lib: &OpenVrLibrary,
    handle: VrOverlayHandle,
    texture: &mut Texture_t,
) -> Result<(), String> {
    let err = unsafe { (lib.overlay().set_overlay_texture)(handle, texture) };
    if err != 0 {
        return Err(format!("EVROverlayError={}", err));
    }
    Ok(())
}
