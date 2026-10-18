//! OpenVR ダッシュボードオーバーレイのライフサイクル管理。Windows専用 (`#[cfg(windows)]` は
//! 呼び出し元の bridge.rs 側でゲートしている)。
//!
//! `render::GlOverlayRenderer` が毎tick描画したegui UIのGLテクスチャを `SetOverlayTexture`
//! でSteamVRコンポジタに渡す。WGLコンテキストはスレッドアフィニティを持つため、
//! renderer の生成・描画・破棄は全てこの `run` 関数を実行するスレッド上で行う。

use std::ffi::{CString, c_char, c_void};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::bridge::{
    ActionView, InputStatus, InputView, OverlayAction, OverlayCommand, OverlayHandle,
    OverlaySnapshot,
};
use super::ffi::{self, OpenVrLibrary, Texture_t, VrOverlayHandle};
use super::input_manifest::{self, InputAction};
use super::manifest;
use super::overlay_ui::{LOGICAL_H, LOGICAL_W, PIXELS_PER_POINT};
use super::render::GlOverlayRenderer;

const OVERLAY_NAME: &str = "VRC Companion";
/// テクスチャの実ピクセル(論理サイズ × PIXELS_PER_POINT。以前は 400×380 を等倍で描いていた)
pub(super) const OVERLAY_WIDTH: u32 = (LOGICAL_W * PIXELS_PER_POINT) as u32;
pub(super) const OVERLAY_HEIGHT: u32 = (LOGICAL_H * PIXELS_PER_POINT) as u32;

pub fn start(initial: OverlaySnapshot, width_m: f32) -> Option<OverlayHandle> {
    let (snapshot_tx, snapshot_rx) = mpsc::channel::<OverlaySnapshot>();
    let (action_tx, action_rx) = mpsc::channel::<OverlayAction>();
    let (command_tx, command_rx) = mpsc::channel::<OverlayCommand>();
    let input_view = Arc::new(Mutex::new(InputView::default()));

    let shared = input_view.clone();
    std::thread::spawn(move || run(initial, width_m, snapshot_rx, action_tx, command_rx, shared));

    Some(OverlayHandle {
        snapshot_tx,
        action_rx,
        command_tx,
        input_view,
    })
}

fn set_view(shared: &Mutex<InputView>, view: &InputView) {
    if let Ok(mut v) = shared.lock()
        && *v != *view
    {
        *v = view.clone();
    }
}

fn run(
    initial: OverlaySnapshot,
    width_m: f32,
    snapshot_rx: Receiver<OverlaySnapshot>,
    action_tx: Sender<OverlayAction>,
    command_rx: Receiver<OverlayCommand>,
    shared_view: Arc<Mutex<InputView>>,
) {
    let lib = match OpenVrLibrary::load() {
        Ok(lib) => lib,
        Err(e) => {
            eprintln!(
                "[SteamVR] 初期化失敗 (SteamVR未起動、またはopenvr_api.dllが見つからない): {}",
                e
            );
            set_view(
                &shared_view,
                &InputView {
                    status: InputStatus::Unavailable("SteamVR が動いていません".to_string()),
                    actions: Vec::new(),
                },
            );
            return;
        }
    };

    // SteamVR Input(ボタン割り当て)用のファイルを先に書く。vrmanifest に場所を書き込むため
    let action_manifest = match input_manifest::default_input_dir()
        .and_then(|dir| input_manifest::write_input_files(&dir))
    {
        Ok(path) => Some(path),
        Err(e) => {
            eprintln!(
                "[SteamVR] アクションマニフェスト書き込み失敗 (ボタン割り当ては無効): {}",
                e
            );
            None
        }
    };

    ensure_auto_launch_registered(&lib, action_manifest.as_deref());

    // SetActionManifestPath は最初の PollNextEvent より前に呼ぶ必要がある(openvr.h)ので、
    // オーバーレイのイベントを読み始める前にここで初期化する
    let mut input = action_manifest.as_deref().and_then(|path| {
        SteamVrInput::init(&lib, path).map(|mut i| {
            i.status_file = path.parent().map(|d| d.join(STATUS_FILE_NAME));
            i
        })
    });
    let mut view = match &input {
        Some(_) => InputView {
            status: InputStatus::Ready,
            actions: Vec::new(),
        },
        None => InputView {
            status: InputStatus::Unavailable("SteamVR Input を初期化できませんでした".to_string()),
            actions: Vec::new(),
        },
    };
    set_view(&shared_view, &view);

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
    // マウス座標は UI の論理ピクセルで受け取る(テクスチャは PIXELS_PER_POINT 倍の解像度)
    if let Err(e) = set_overlay_mouse_scale(&lib, overlay_handle, LOGICAL_W, LOGICAL_H) {
        eprintln!("[SteamVR] SetOverlayMouseScale 失敗: {}", e);
    }
    // 表示の大きさ(幅)。呼ばないと SteamVR の既定の 1m になり、小さくて読みにくかった
    if let Err(e) = set_overlay_width_in_meters(&lib, overlay_handle, width_m) {
        eprintln!("[SteamVR] SetOverlayWidthInMeters 失敗: {}", e);
    }

    let mut renderer = match GlOverlayRenderer::new(OVERLAY_WIDTH, OVERLAY_HEIGHT, PIXELS_PER_POINT)
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[SteamVR] GlOverlayRenderer 初期化失敗: {}", e);
            return;
        }
    };

    let mut latest = initial;
    loop {
        // WaitFrameSync でコンポジタの次フレームに同期する(固定インターバルの
        // recv_timeoutによるポーリングではなく、コンポジタ側のペースに揃える)。
        // タイムアウトしても(戻り値がエラーでも)描画自体は続ける — SteamVRが
        // 一時的にフレームを配れない状況(例: ダッシュボード非表示中)でも
        // オーバーレイの更新を止めたくないため。
        if let Err(e) = wait_frame_sync(&lib, 100) {
            eprintln!("[SteamVR] WaitFrameSync 失敗: {}", e);
        }

        match snapshot_rx.try_recv() {
            Ok(s) => latest = s,
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => break,
        }

        // SteamVR のバインドで割り当てられたボタン操作(VRChat を操作中でも届く)
        if let Some(input) = input.as_mut() {
            for action in input.poll(&lib) {
                let _ = action_tx.send(OverlayAction::from_input(action));
            }
            input.refresh_bindings_if_due(&lib);
            view = input.view();
            set_view(&shared_view, &view);
        }
        latest.input = view.clone();

        // デスクトップの設定画面からの頼みごと
        while let Ok(cmd) = command_rx.try_recv() {
            match cmd {
                OverlayCommand::OpenBindingUi { on_desktop } => match input.as_ref() {
                    Some(i) => i.open_binding_ui(&lib, on_desktop),
                    None => {
                        eprintln!("[SteamVR] SteamVR Input が使えないので割り当て画面を開けない")
                    }
                },
            }
        }

        let events = poll_overlay_events(&lib, overlay_handle);
        match renderer.render(&latest, events) {
            Ok((texture, actions)) => {
                if !actions.is_empty() {
                    if let Err(e) = trigger_laser_mouse_haptic_vibration(&lib, overlay_handle) {
                        eprintln!("[SteamVR] TriggerLaserMouseHapticVibration 失敗: {}", e);
                    }
                }
                for action in actions {
                    if action == OverlayAction::OpenBindingUi {
                        // VR の中(ダッシュボード)で SteamVR の割り当て画面を開く
                        match input.as_ref() {
                            Some(i) => i.open_binding_ui(&lib, false),
                            None => eprintln!(
                                "[SteamVR] SteamVR Input が使えないので割り当て画面を開けない"
                            ),
                        }
                        continue;
                    }
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

/// SteamVR起動時にこのexeを自動起動してもらうよう、アプリケーションマニフェストを登録する。
/// IVRApplicationsインターフェース取得失敗・マニフェスト書き込み失敗はいずれも非致命的に扱い、
/// オーバーレイ本体の起動をブロックしない(既存のSetOverlayInputMethod等と同じパターン)。
fn ensure_auto_launch_registered(lib: &OpenVrLibrary, action_manifest: Option<&std::path::Path>) {
    let Some(applications) = lib.applications() else {
        return;
    };

    let path = match manifest::write_manifest(action_manifest) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("[SteamVR] マニフェスト書き込み失敗: {}", e);
            return;
        }
    };

    let path_c = match CString::new(path.to_string_lossy().into_owned()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[SteamVR] マニフェストパスのCString化失敗: {}", e);
            return;
        }
    };
    let err =
        unsafe { (applications.add_application_manifest)(path_c.as_ptr() as *mut c_char, false) };
    if err != 0 {
        eprintln!(
            "[SteamVR] AddApplicationManifest 失敗: EVRApplicationError={}",
            err
        );
        return;
    }

    let key = CString::new(manifest::APP_KEY).unwrap();

    // 手で起動した(SteamVR の自動起動でない)場合、SteamVR はこのプロセスがどのアプリか
    // 分からず、ボタン割り当てが別アプリ扱いになる。自分で名乗っておく(0 = 自分自身)。
    // SteamVR から起動された場合はすでに分かっているのでエラーが返ることがあるが、問題ない
    let err = unsafe { (applications.identify_application)(0, key.as_ptr() as *mut c_char) };
    if err != 0 {
        eprintln!(
            "[SteamVR] IdentifyApplication: EVRApplicationError={} (SteamVR から起動された場合は無視してよい)",
            err
        );
    }

    let err =
        unsafe { (applications.set_application_auto_launch)(key.as_ptr() as *mut c_char, true) };
    if err != 0 {
        eprintln!(
            "[SteamVR] SetApplicationAutoLaunch 失敗: EVRApplicationError={}",
            err
        );
    }
}

/// SteamVR Input(IVRInput)でボタン操作を受け取る。
///
/// このアプリは Overlay アプリなので、VRChat(シーンアプリ)が動いている間もアクションが届く。
/// 優先度 0 で使うので VRChat 側の入力は奪わない(同じ押下が両方に届く)。
/// SteamVR のダッシュボードを開いている間は届かない(SteamVR の仕様)。
struct SteamVrInput {
    action_set: ffi::VRActionSetHandle_t,
    actions: Vec<(InputAction, ffi::VRActionHandle_t)>,
    warned: bool,
    /// actions と同じ並び: いま割り当てられているボタン / 最後に押された時刻
    bindings: Vec<Vec<String>>,
    fired_at: Vec<Option<Instant>>,
    last_refresh: Option<Instant>,
    /// 割り当てが変わったら書き出す(動作確認用。%APPDATA%\vrc-companion\steamvr_input\current_bindings.txt)
    status_file: Option<std::path::PathBuf>,
}

const STATUS_FILE_NAME: &str = "current_bindings.txt";
/// 押されたあと「反応」を点けておく時間
const FIRED_SHOW: Duration = Duration::from_millis(1500);
/// 割り当てを SteamVR に問い合わせ直す間隔(バインド画面で変えたらすぐ反映されるように)
const REFRESH_EVERY: Duration = Duration::from_millis(500);

impl SteamVrInput {
    fn init(lib: &OpenVrLibrary, manifest_path: &std::path::Path) -> Option<Self> {
        let input = lib.input()?;
        let path_c = CString::new(manifest_path.to_string_lossy().into_owned()).ok()?;
        let err = unsafe { (input.set_action_manifest_path)(path_c.as_ptr() as *mut c_char) };
        if err != 0 {
            eprintln!(
                "[SteamVR] SetActionManifestPath 失敗: EVRInputError={}",
                err
            );
            return None;
        }

        let set_c = CString::new(input_manifest::ACTION_SET).unwrap();
        let mut action_set: ffi::VRActionSetHandle_t = 0;
        let err = unsafe {
            (input.get_action_set_handle)(set_c.as_ptr() as *mut c_char, &mut action_set)
        };
        if err != 0 {
            eprintln!("[SteamVR] GetActionSetHandle 失敗: EVRInputError={}", err);
            return None;
        }

        let mut actions = Vec::new();
        for action in InputAction::ALL {
            let name = CString::new(action.path()).unwrap();
            let mut handle: ffi::VRActionHandle_t = 0;
            let err =
                unsafe { (input.get_action_handle)(name.as_ptr() as *mut c_char, &mut handle) };
            if err != 0 {
                eprintln!(
                    "[SteamVR] GetActionHandle({}) 失敗: EVRInputError={}",
                    action.path(),
                    err
                );
                continue;
            }
            actions.push((action, handle));
        }
        eprintln!(
            "[SteamVR] ボタン割り当て(SteamVR Input)を有効化: {}",
            manifest_path.display()
        );
        let n = actions.len();
        Some(Self {
            action_set,
            actions,
            warned: false,
            bindings: vec![Vec::new(); n],
            fired_at: vec![None; n],
            last_refresh: None,
            status_file: None,
        })
    }

    fn view(&self) -> InputView {
        InputView {
            status: InputStatus::Ready,
            actions: self
                .actions
                .iter()
                .enumerate()
                .map(|(i, (action, _))| ActionView {
                    label: action.label_ja(),
                    bindings: self.bindings[i].clone(),
                    fired: self.fired_at[i].is_some_and(|t| t.elapsed() < FIRED_SHOW),
                })
                .collect(),
        }
    }

    /// いま割り当てられているボタンを SteamVR に聞く(UpdateActionState のあとに呼ぶ)
    fn refresh_bindings_if_due(&mut self, lib: &OpenVrLibrary) {
        if self
            .last_refresh
            .is_some_and(|t| t.elapsed() < REFRESH_EVERY)
        {
            return;
        }
        self.last_refresh = Some(Instant::now());
        let Some(input) = lib.input() else {
            return;
        };
        let mut changed = false;
        for (i, &(_, handle)) in self.actions.iter().enumerate() {
            let names = query_bindings(input, self.action_set, handle);
            if names != self.bindings[i] {
                self.bindings[i] = names;
                changed = true;
            }
        }
        if changed {
            self.write_status_file();
        }
    }

    fn write_status_file(&self) {
        let Some(path) = &self.status_file else {
            return;
        };
        let mut text = String::new();
        for (i, (action, _)) in self.actions.iter().enumerate() {
            let b = if self.bindings[i].is_empty() {
                "未割り当て".to_string()
            } else {
                self.bindings[i].join(" / ")
            };
            text.push_str(&format!("{}: {}\r\n", action.label_ja(), b));
        }
        if let Err(e) = std::fs::write(path, &text) {
            eprintln!("[SteamVR] {} の書き込み失敗: {}", path.display(), e);
        } else {
            eprintln!(
                "[SteamVR] ボタン割り当て: {}",
                text.trim_end().replace("\r\n", " | ")
            );
        }
    }

    /// SteamVR のボタン割り当て画面をこのアプリのページで開く。
    /// on_desktop = true なら PC の画面、false なら VR の中(ダッシュボード)
    fn open_binding_ui(&self, lib: &OpenVrLibrary, on_desktop: bool) {
        let Some(input) = lib.input() else {
            return;
        };
        let key = CString::new(manifest::APP_KEY).unwrap();
        let err = unsafe {
            (input.open_binding_ui)(
                key.as_ptr() as *mut c_char,
                self.action_set,
                ffi::INVALID_INPUT_VALUE_HANDLE,
                on_desktop,
            )
        };
        if err != 0 {
            eprintln!(
                "[SteamVR] OpenBindingUI(on_desktop={}) 失敗: EVRInputError={}",
                on_desktop, err
            );
        } else {
            eprintln!(
                "[SteamVR] OpenBindingUI(on_desktop={}) を呼んだ",
                on_desktop
            );
        }
    }

    /// 前回から「押された」(離された→押された)アクションを返す。毎フレーム呼ぶ
    fn poll(&mut self, lib: &OpenVrLibrary) -> Vec<InputAction> {
        let Some(input) = lib.input() else {
            return Vec::new();
        };
        let mut active = ffi::VRActiveActionSet_t {
            ul_action_set: self.action_set,
            ul_restricted_to_device: ffi::INVALID_INPUT_VALUE_HANDLE,
            ul_secondary_action_set: 0,
            un_padding: 0,
            n_priority: 0,
        };
        let err = unsafe {
            (input.update_action_state)(
                &mut active,
                std::mem::size_of::<ffi::VRActiveActionSet_t>() as u32,
                1,
            )
        };
        if err != 0 {
            if !self.warned {
                eprintln!("[SteamVR] UpdateActionState 失敗: EVRInputError={}", err);
                self.warned = true;
            }
            return Vec::new();
        }

        let mut pressed = Vec::new();
        for &(action, handle) in &self.actions {
            let mut data = ffi::InputDigitalActionData_t::default();
            let err = unsafe {
                (input.get_digital_action_data)(
                    handle,
                    &mut data,
                    std::mem::size_of::<ffi::InputDigitalActionData_t>() as u32,
                    ffi::INVALID_INPUT_VALUE_HANDLE,
                )
            };
            if err == 0 && data.b_active && data.b_changed && data.b_state {
                pressed.push(action);
            }
        }
        for a in &pressed {
            if let Some(i) = self.actions.iter().position(|(x, _)| x == a) {
                self.fired_at[i] = Some(Instant::now());
            }
            eprintln!("[SteamVR] ボタン操作: {}", a.label_ja());
        }
        pressed
    }
}

/// 1つの操作に割り当てられているボタンの説明。GetActionBindingInfo(押し方まで分かる)を使い、
/// 取れなければ GetActionOrigins + GetOriginLocalizedName(SteamVR の言語でのボタン名)
fn query_bindings(
    input: &ffi::VrIvInputFnTable,
    action_set: ffi::VRActionSetHandle_t,
    handle: ffi::VRActionHandle_t,
) -> Vec<String> {
    let mut infos: Vec<ffi::InputBindingInfo_t> = (0..ffi::MAX_ACTION_ORIGIN_COUNT)
        .map(|_| unsafe { std::mem::zeroed() })
        .collect();
    let mut count: u32 = 0;
    let err = unsafe {
        (input.get_action_binding_info)(
            handle,
            infos.as_mut_ptr(),
            std::mem::size_of::<ffi::InputBindingInfo_t>() as u32,
            infos.len() as u32,
            &mut count,
        )
    };
    let mut names: Vec<String> = Vec::new();
    if err == 0 {
        for info in infos.iter().take(count as usize) {
            let name = input_manifest::describe_binding(
                &ffi::c_buf_to_string(&info.rch_device_path_name),
                &ffi::c_buf_to_string(&info.rch_input_path_name),
                &ffi::c_buf_to_string(&info.rch_slot_name),
            );
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    if !names.is_empty() {
        return names;
    }

    let mut origins = [0 as ffi::VRInputValueHandle_t; ffi::MAX_ACTION_ORIGIN_COUNT];
    let err = unsafe {
        (input.get_action_origins)(
            action_set,
            handle,
            origins.as_mut_ptr(),
            origins.len() as u32,
        )
    };
    if err != 0 {
        return names;
    }
    for &origin in origins
        .iter()
        .filter(|&&o| o != ffi::INVALID_INPUT_VALUE_HANDLE)
    {
        let mut buf = [0u8; 256];
        let err = unsafe {
            (input.get_origin_localized_name)(
                origin,
                buf.as_mut_ptr() as *mut c_char,
                buf.len() as u32,
                ffi::INPUT_STRING_HAND_AND_SOURCE,
            )
        };
        if err == 0 {
            let name = ffi::c_buf_to_string(&buf);
            if !name.is_empty() && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

fn create_dashboard_overlay(lib: &OpenVrLibrary) -> Result<VrOverlayHandle, String> {
    let key = CString::new(manifest::APP_KEY).unwrap();
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

fn set_overlay_width_in_meters(
    lib: &OpenVrLibrary,
    handle: VrOverlayHandle,
    width_m: f32,
) -> Result<(), String> {
    let err = unsafe { (lib.overlay().set_overlay_width_in_meters)(handle, width_m) };
    if err != 0 {
        return Err(format!("EVROverlayError={}", err));
    }
    Ok(())
}

fn wait_frame_sync(lib: &OpenVrLibrary, timeout_ms: u32) -> Result<(), String> {
    let err = unsafe { (lib.overlay().wait_frame_sync)(timeout_ms) };
    if err != 0 {
        return Err(format!("EVROverlayError={}", err));
    }
    Ok(())
}

/// クリック成立時にレーザーポインタへ短い振動フィードバックを返す。
/// 継続時間・周波数・振幅は「トリガー押下がUIに反映された」ことが分かる程度の
/// 短いパルスとして適当に選んだ値(実機での感触調整が必要な可能性あり)。
fn trigger_laser_mouse_haptic_vibration(
    lib: &OpenVrLibrary,
    handle: VrOverlayHandle,
) -> Result<(), String> {
    let err =
        unsafe { (lib.overlay().trigger_laser_mouse_haptic_vibration)(handle, 0.05, 40.0, 1.0) };
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
