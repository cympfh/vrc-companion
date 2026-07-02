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
    action_tx: Sender<OverlayAction>,
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

    // SetOverlayInputMethod を呼ばない限りコンポジタはこのオーバーレイに対して
    // MouseMove/MouseButtonDown/Up を一切生成しない(デフォルトはInputMethod_None)。
    // これが無いと表示だけされてレーザーポインタ操作が効かない。
    if let Err(e) = set_overlay_input_method(&lib, overlay_handle) {
        eprintln!("[SteamVR] SetOverlayInputMethod 失敗: {}", e);
    }
    if let Err(e) = set_overlay_mouse_scale(
        &lib,
        overlay_handle,
        OVERLAY_WIDTH as f32,
        OVERLAY_HEIGHT as f32,
    ) {
        eprintln!("[SteamVR] SetOverlayMouseScale 失敗: {}", e);
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

        let events = poll_overlay_events(&lib, overlay_handle);
        match renderer.render(&latest, events) {
            Ok((texture, actions)) => {
                for action in actions {
                    let _ = action_tx.send(action);
                }
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

/// `PollNextOverlayEvent` を空になるまでポーリングし、マウス関連イベントを
/// `egui::Event` に変換する。
///
/// - Y座標: 実機検証の結果、OpenVRのオーバーレイマウス座標は反転不要(既にegui同様の
///   原点左上・Y下向き)だったため、`mouse.y`をそのまま使う(以前あった`height - mouse.y`
///   の反転は誤りだった)。
/// - ボタン: ダッシュボードのレーザーポインタ操作は実質「トリガー押下=クリック」の
///   単一ボタン操作であり、UI側(`render.rs`)も`Response::clicked()`(Primaryボタン限定)
///   しか見ていない。`VREvent_Mouse_t.button`のビット値解釈に依存すると、離す側の
///   イベントでこのフィールドが押す側と一致しない/正しく載らない実装がある場合に
///   クリックとして成立しなくなるため、ボタン種別に関わらず常にPrimaryとして扱う。
fn poll_overlay_events(lib: &OpenVrLibrary, handle: VrOverlayHandle) -> Vec<egui::Event> {
    let mut events = Vec::new();
    loop {
        let mut vr_event: ffi::VREvent_t = unsafe { std::mem::zeroed() };
        let has_event = unsafe {
            (lib.overlay().poll_next_overlay_event)(
                handle,
                &mut vr_event,
                std::mem::size_of::<ffi::VREvent_t>() as u32,
            )
        };
        if !has_event {
            break;
        }

        if matches!(
            vr_event.event_type,
            ffi::EVENT_MOUSE_MOVE | ffi::EVENT_MOUSE_BUTTON_DOWN | ffi::EVENT_MOUSE_BUTTON_UP
        ) {
            let mouse = unsafe { vr_event.data.mouse };
            let pos = egui::pos2(mouse.x, mouse.y);
            events.push(egui::Event::PointerMoved(pos));

            match vr_event.event_type {
                ffi::EVENT_MOUSE_BUTTON_DOWN => events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                }),
                ffi::EVENT_MOUSE_BUTTON_UP => events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                }),
                _ => {}
            }
        }
    }
    events
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

fn set_overlay_input_method(lib: &OpenVrLibrary, handle: VrOverlayHandle) -> Result<(), String> {
    let err = unsafe { (lib.overlay().set_overlay_input_method)(handle, ffi::INPUT_METHOD_MOUSE) };
    if err != 0 {
        return Err(format!("EVROverlayError={}", err));
    }
    Ok(())
}

fn set_overlay_mouse_scale(
    lib: &OpenVrLibrary,
    handle: VrOverlayHandle,
    width: f32,
    height: f32,
) -> Result<(), String> {
    let mut scale = ffi::HmdVector2_t { v: [width, height] };
    let err = unsafe { (lib.overlay().set_overlay_mouse_scale)(handle, &mut scale) };
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
