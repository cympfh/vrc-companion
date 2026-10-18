//! OpenVR プレーンC API (openvr_api.dll) への生FFIバインディング。
//!
//! 公式OpenVR C++ SDKはMSVCビルドでvtable ABIがこのプロジェクトのクロスコンパイル先
//! (x86_64-pc-windows-gnu) と非互換なため使えない。代わりに `libloading` で
//! `openvr_api.dll` を動的ロードし、`VR_GetGenericInterface("FnTable:IVROverlay_028", ...)`
//! で ABI安全な関数ポインタテーブルを取得する。
//!
//! フィールドの型・順序は公式ヘッダ
//! https://github.com/ValveSoftware/openvr/blob/master/headers/openvr_capi.h
//! (取得日 2026-07-01、IVRApplicationsは2026-07-02) から逐語的に転記した。
//! 順序はABIクリティカル — 変更しないこと。
//! 実際に呼び出さないフィールドは、正しい順序・サイズを保つためのプレースホルダ
//! (`PlaceholderFn`) にしている。

use libloading::Library;
use std::ffi::{c_char, c_void};

pub const IVROVERLAY_VERSION: &str = "IVROverlay_028";
pub const IVRAPPLICATIONS_VERSION: &str = "IVRApplications_008";
/// 新しい順に試す。IVRInput_010 と 011 の違いは 011 で末尾に関数が足されたことだけで、
/// ここで呼ぶ関数(SetActionManifestPath 〜 OpenBindingUI)は同じ位置にある。
pub const IVRINPUT_VERSIONS: [&str; 2] = ["IVRInput_011", "IVRInput_010"];
const VR_APPLICATION_TYPE_OVERLAY: i32 = 2;

pub type VrOverlayHandle = u64;
pub type EVRInitError = i32;
pub type EVROverlayError = i32;
pub type EVRApplicationError = i32;
pub type EVRInputError = i32;
pub const INPUT_ERROR_IPC: EVRInputError = 7;
#[allow(non_camel_case_types)]
pub type VRActionHandle_t = u64;
#[allow(non_camel_case_types)]
pub type VRActionSetHandle_t = u64;
#[allow(non_camel_case_types)]
pub type VRInputValueHandle_t = u64;
pub const INVALID_INPUT_VALUE_HANDLE: VRInputValueHandle_t = 0;

/// `VRActiveActionSet_t`(openvr.h)。nPriority は 0(通常)で使う — 0 ならゲーム(VRChat)の
/// 入力を奪わず、同じボタン押下がゲームとこのアプリの両方に届く。
#[repr(C)]
pub struct VRActiveActionSet_t {
    pub ul_action_set: VRActionSetHandle_t,
    pub ul_restricted_to_device: VRInputValueHandle_t,
    pub ul_secondary_action_set: VRActionSetHandle_t,
    pub un_padding: u32,
    pub n_priority: i32,
}

/// アクションに割り当てられたボタンの数の上限(k_unMaxActionOriginCount)
pub const MAX_ACTION_ORIGIN_COUNT: usize = 16;
/// GetOriginLocalizedName: 手(左手/右手) + ボタン名
pub const INPUT_STRING_HAND_AND_SOURCE: i32 = 0x01 | 0x04;

/// `InputBindingInfo_t`(openvr.h)。例: "/user/hand/left", "/input/dpad_down", "button", "double"
#[repr(C)]
pub struct InputBindingInfo_t {
    pub rch_device_path_name: [u8; 128],
    pub rch_input_path_name: [u8; 128],
    pub rch_mode_name: [u8; 128],
    pub rch_slot_name: [u8; 128],
    pub rch_input_source_type: [u8; 32],
}

/// NUL 終端の C 文字列バッファ → String
pub fn c_buf_to_string(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// `InputDigitalActionData_t`(openvr.h、Windows は pack 8)
#[repr(C)]
#[derive(Default)]
pub struct InputDigitalActionData_t {
    pub b_active: bool,
    pub active_origin: VRInputValueHandle_t,
    pub b_state: bool,
    pub b_changed: bool,
    pub f_update_time: f32,
}

pub type ETextureType = i32;
pub const TEXTURE_TYPE_OPENGL: ETextureType = 1;
pub type EColorSpace = i32;
pub const COLOR_SPACE_AUTO: EColorSpace = 0;

#[repr(C)]
pub struct Texture_t {
    pub handle: *mut c_void,
    pub e_type: ETextureType,
    pub e_color_space: EColorSpace,
}

pub const EVENT_MOUSE_MOVE: u32 = 300;
pub const EVENT_MOUSE_BUTTON_DOWN: u32 = 301;
pub const EVENT_MOUSE_BUTTON_UP: u32 = 302;

pub type EVROverlayInputMethod = i32;
pub const INPUT_METHOD_MOUSE: EVROverlayInputMethod = 1;

#[repr(C)]
pub struct HmdVector2_t {
    pub v: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VREvent_Mouse_t {
    pub x: f32,
    pub y: f32,
    pub button: u32,
    pub cursor_index: u32,
}

/// C側の32メンバーunionのうち、Stage3で実際に使う`mouse`と、サイズ/アライメントを
/// 本物(最大メンバーである`VREvent_Reserved_t` = 6×uint64_t, 48バイト, align 8)に
/// 揃えるための`reserved`のみを転記する。
#[repr(C)]
#[derive(Clone, Copy)]
pub union VREvent_Data_t {
    pub reserved: [u64; 6],
    pub mouse: VREvent_Mouse_t,
}

#[repr(C)]
pub struct VREvent_t {
    pub event_type: u32,
    pub tracked_device_index: u32,
    pub event_age_seconds: f32,
    pub data: VREvent_Data_t,
}

/// 実際には呼び出さないフィールド用のプレースホルダ型。
/// C関数ポインタと同サイズ(8バイト/64bit)であることだけが目的。
type PlaceholderFn = Option<unsafe extern "C" fn()>;

#[repr(C)]
pub struct VrIvOverlayFnTable {
    pub find_overlay: PlaceholderFn,
    pub create_overlay: PlaceholderFn,
    pub create_subview_overlay: PlaceholderFn,
    pub destroy_overlay: unsafe extern "C" fn(VrOverlayHandle) -> EVROverlayError,
    pub get_overlay_key: PlaceholderFn,
    pub get_overlay_name: PlaceholderFn,
    pub set_overlay_name: PlaceholderFn,
    pub get_overlay_image_data: PlaceholderFn,
    pub get_overlay_error_name_from_enum: PlaceholderFn,
    pub set_overlay_rendering_pid: PlaceholderFn,
    pub get_overlay_rendering_pid: PlaceholderFn,
    pub set_overlay_flag: PlaceholderFn,
    pub get_overlay_flag: PlaceholderFn,
    pub get_overlay_flags: PlaceholderFn,
    pub set_overlay_color: PlaceholderFn,
    pub get_overlay_color: PlaceholderFn,
    pub set_overlay_alpha: PlaceholderFn,
    pub get_overlay_alpha: PlaceholderFn,
    pub set_overlay_texel_aspect: PlaceholderFn,
    pub get_overlay_texel_aspect: PlaceholderFn,
    pub set_overlay_sort_order: PlaceholderFn,
    pub get_overlay_sort_order: PlaceholderFn,
    pub set_overlay_width_in_meters: unsafe extern "C" fn(VrOverlayHandle, f32) -> EVROverlayError,
    pub get_overlay_width_in_meters: PlaceholderFn,
    pub set_overlay_curvature: PlaceholderFn,
    pub get_overlay_curvature: PlaceholderFn,
    pub set_overlay_pre_curve_pitch: PlaceholderFn,
    pub get_overlay_pre_curve_pitch: PlaceholderFn,
    pub set_overlay_texture_color_space: PlaceholderFn,
    pub get_overlay_texture_color_space: PlaceholderFn,
    pub set_overlay_texture_bounds: PlaceholderFn,
    pub get_overlay_texture_bounds: PlaceholderFn,
    pub get_overlay_transform_type: PlaceholderFn,
    pub set_overlay_transform_absolute: PlaceholderFn,
    pub get_overlay_transform_absolute: PlaceholderFn,
    pub set_overlay_transform_tracked_device_relative: PlaceholderFn,
    pub get_overlay_transform_tracked_device_relative: PlaceholderFn,
    pub set_overlay_transform_tracked_device_component: PlaceholderFn,
    pub get_overlay_transform_tracked_device_component: PlaceholderFn,
    pub set_overlay_transform_cursor: PlaceholderFn,
    pub get_overlay_transform_cursor: PlaceholderFn,
    pub set_overlay_transform_projection: PlaceholderFn,
    pub set_subview_position: PlaceholderFn,
    pub show_overlay: unsafe extern "C" fn(VrOverlayHandle) -> EVROverlayError,
    pub hide_overlay: PlaceholderFn,
    pub is_overlay_visible: unsafe extern "C" fn(VrOverlayHandle) -> bool,
    pub get_transform_for_overlay_coordinates: PlaceholderFn,
    pub wait_frame_sync: unsafe extern "C" fn(u32) -> EVROverlayError,
    pub poll_next_overlay_event: unsafe extern "C" fn(VrOverlayHandle, *mut VREvent_t, u32) -> bool,
    pub get_overlay_input_method: PlaceholderFn,
    pub set_overlay_input_method:
        unsafe extern "C" fn(VrOverlayHandle, EVROverlayInputMethod) -> EVROverlayError,
    pub get_overlay_mouse_scale: PlaceholderFn,
    pub set_overlay_mouse_scale:
        unsafe extern "C" fn(VrOverlayHandle, *mut HmdVector2_t) -> EVROverlayError,
    pub compute_overlay_intersection: PlaceholderFn,
    pub is_hover_target_overlay: PlaceholderFn,
    pub set_overlay_intersection_mask: PlaceholderFn,
    pub trigger_laser_mouse_haptic_vibration:
        unsafe extern "C" fn(VrOverlayHandle, f32, f32, f32) -> EVROverlayError,
    pub set_overlay_cursor: PlaceholderFn,
    pub set_overlay_cursor_position_override: PlaceholderFn,
    pub clear_overlay_cursor_position_override: PlaceholderFn,
    pub set_overlay_texture:
        unsafe extern "C" fn(VrOverlayHandle, *mut Texture_t) -> EVROverlayError,
    pub clear_overlay_texture: PlaceholderFn,
    pub set_overlay_raw:
        unsafe extern "C" fn(VrOverlayHandle, *mut c_void, u32, u32, u32) -> EVROverlayError,
    pub set_overlay_from_file: PlaceholderFn,
    pub get_overlay_texture: PlaceholderFn,
    pub release_native_overlay_handle: PlaceholderFn,
    pub get_overlay_texture_size: PlaceholderFn,
    pub create_dashboard_overlay: unsafe extern "C" fn(
        *mut c_char,
        *mut c_char,
        *mut VrOverlayHandle,
        *mut VrOverlayHandle,
    ) -> EVROverlayError,
    pub is_dashboard_visible: PlaceholderFn,
    pub is_active_dashboard_overlay: PlaceholderFn,
    pub set_dashboard_overlay_scene_process: PlaceholderFn,
    pub get_dashboard_overlay_scene_process: PlaceholderFn,
    pub show_dashboard: PlaceholderFn,
    pub get_primary_dashboard_device: PlaceholderFn,
    pub show_keyboard: PlaceholderFn,
    pub show_keyboard_for_overlay: PlaceholderFn,
    pub get_keyboard_text: PlaceholderFn,
    pub hide_keyboard: PlaceholderFn,
    pub set_keyboard_transform_absolute: PlaceholderFn,
    pub set_keyboard_position_for_overlay: PlaceholderFn,
    pub show_message_overlay: PlaceholderFn,
    pub close_message_overlay: PlaceholderFn,
}

/// `VR_IVROverlay_FnTable` の総フィールド数(公式ヘッダより逐語転記で確認した数)。
/// フィールドの追加・削除ミスを検知するトリップワイヤー。
#[cfg(test)]
const OVERLAY_FN_TABLE_FIELD_COUNT: usize = 82;

#[repr(C)]
pub struct VrIvApplicationsFnTable {
    pub add_application_manifest: unsafe extern "C" fn(*mut c_char, bool) -> EVRApplicationError,
    pub remove_application_manifest: PlaceholderFn,
    pub is_application_installed: unsafe extern "C" fn(*mut c_char) -> bool,
    pub get_application_count: PlaceholderFn,
    pub get_application_key_by_index: PlaceholderFn,
    pub get_application_key_by_process_id: PlaceholderFn,
    pub launch_application: PlaceholderFn,
    pub launch_template_application: PlaceholderFn,
    pub launch_application_from_mime_type: PlaceholderFn,
    pub launch_dashboard_overlay: PlaceholderFn,
    pub cancel_application_launch: PlaceholderFn,
    pub identify_application: unsafe extern "C" fn(u32, *mut c_char) -> EVRApplicationError,
    pub get_application_process_id: PlaceholderFn,
    pub get_applications_error_name_from_enum: PlaceholderFn,
    pub get_application_property_string: PlaceholderFn,
    pub get_application_property_bool: PlaceholderFn,
    pub get_application_property_uint64: PlaceholderFn,
    pub set_application_auto_launch: unsafe extern "C" fn(*mut c_char, bool) -> EVRApplicationError,
    pub get_application_auto_launch: unsafe extern "C" fn(*mut c_char) -> bool,
    pub set_default_application_for_mime_type: PlaceholderFn,
    pub get_default_application_for_mime_type: PlaceholderFn,
    pub get_application_supported_mime_types: PlaceholderFn,
    pub get_applications_that_support_mime_type: PlaceholderFn,
    pub get_application_launch_arguments: PlaceholderFn,
    pub get_starting_application: PlaceholderFn,
    pub get_scene_application_state: PlaceholderFn,
    pub perform_application_prelaunch_check: PlaceholderFn,
    pub get_scene_application_state_name_from_enum: PlaceholderFn,
    pub launch_internal_process: PlaceholderFn,
    pub register_subprocess: PlaceholderFn,
    pub get_current_scene_process_id: PlaceholderFn,
}

/// `VR_IVRApplications_FnTable` の総フィールド数(公式ヘッダより逐語転記で確認した数)。
#[cfg(test)]
const APPLICATIONS_FN_TABLE_FIELD_COUNT: usize = 31;

/// `VR_IVRInput_FnTable`(openvr_capi.h、取得日 2026-10-18、IVRInput_011)。
/// 呼ぶのは先頭6つだけ。残りは並びとサイズを保つためのプレースホルダ。
#[repr(C)]
pub struct VrIvInputFnTable {
    pub set_action_manifest_path: unsafe extern "C" fn(*mut c_char) -> EVRInputError,
    pub get_action_set_handle:
        unsafe extern "C" fn(*mut c_char, *mut VRActionSetHandle_t) -> EVRInputError,
    pub get_action_handle:
        unsafe extern "C" fn(*mut c_char, *mut VRActionHandle_t) -> EVRInputError,
    pub get_input_source_handle: PlaceholderFn,
    pub update_action_state:
        unsafe extern "C" fn(*mut VRActiveActionSet_t, u32, u32) -> EVRInputError,
    pub get_digital_action_data: unsafe extern "C" fn(
        VRActionHandle_t,
        *mut InputDigitalActionData_t,
        u32,
        VRInputValueHandle_t,
    ) -> EVRInputError,
    pub get_analog_action_data: PlaceholderFn,
    pub get_pose_action_data_relative_to_now: PlaceholderFn,
    pub get_pose_action_data_for_next_frame: PlaceholderFn,
    pub get_skeletal_action_data: PlaceholderFn,
    pub get_dominant_hand: PlaceholderFn,
    pub set_dominant_hand: PlaceholderFn,
    pub get_eye_tracking_data_relative_to_now: PlaceholderFn,
    pub get_eye_tracking_data_for_next_frame: PlaceholderFn,
    pub get_bone_count: PlaceholderFn,
    pub get_bone_hierarchy: PlaceholderFn,
    pub get_bone_name: PlaceholderFn,
    pub get_skeletal_reference_transforms: PlaceholderFn,
    pub get_skeletal_tracking_level: PlaceholderFn,
    pub get_skeletal_bone_data: PlaceholderFn,
    pub get_skeletal_summary_data: PlaceholderFn,
    pub get_skeletal_bone_data_compressed: PlaceholderFn,
    pub decompress_skeletal_bone_data: PlaceholderFn,
    pub trigger_haptic_vibration_action: PlaceholderFn,
    pub get_action_origins: unsafe extern "C" fn(
        VRActionSetHandle_t,
        VRActionHandle_t,
        *mut VRInputValueHandle_t,
        u32,
    ) -> EVRInputError,
    pub get_origin_localized_name:
        unsafe extern "C" fn(VRInputValueHandle_t, *mut c_char, u32, i32) -> EVRInputError,
    pub get_origin_tracked_device_info: PlaceholderFn,
    pub get_action_binding_info: unsafe extern "C" fn(
        VRActionHandle_t,
        *mut InputBindingInfo_t,
        u32,
        u32,
        *mut u32,
    ) -> EVRInputError,
    pub show_action_origins: PlaceholderFn,
    pub show_bindings_for_action_set: PlaceholderFn,
    pub get_component_state_for_binding: PlaceholderFn,
    pub is_using_legacy_input: PlaceholderFn,
    pub open_binding_ui: unsafe extern "C" fn(
        *mut c_char,
        VRActionSetHandle_t,
        VRInputValueHandle_t,
        bool,
    ) -> EVRInputError,
    pub get_binding_variant: PlaceholderFn,
}

#[cfg(test)]
const INPUT_FN_TABLE_FIELD_COUNT: usize = 34;

/// openvr_api.dll をロードし、IVROverlay の関数ポインタテーブルを保持する。
/// `_lib` が生きている間だけ `overlay_table` は有効。
pub struct OpenVrLibrary {
    _lib: Library,
    overlay_table: *const VrIvOverlayFnTable,
    applications_table: Option<*const VrIvApplicationsFnTable>,
    input_table: Option<*const VrIvInputFnTable>,
}

impl OpenVrLibrary {
    pub fn load() -> Result<Self, String> {
        let lib = unsafe { Library::new("openvr_api.dll") }
            .map_err(|e| format!("DLLロード失敗: {}", e))?;

        let vr_init_internal: libloading::Symbol<
            unsafe extern "C" fn(*mut EVRInitError, i32) -> isize,
        > = unsafe { lib.get(b"VR_InitInternal\0") }
            .map_err(|e| format!("VR_InitInternal が見つからない: {}", e))?;

        let mut init_error: EVRInitError = 0;
        let token = unsafe { vr_init_internal(&mut init_error, VR_APPLICATION_TYPE_OVERLAY) };
        if init_error != 0 || token == 0 {
            return Err(format!("VR_InitInternal エラー: code={}", init_error));
        }

        let vr_get_generic_interface: libloading::Symbol<
            unsafe extern "C" fn(*const c_char, *mut EVRInitError) -> isize,
        > = unsafe { lib.get(b"VR_GetGenericInterface\0") }
            .map_err(|e| format!("VR_GetGenericInterface が見つからない: {}", e))?;

        let version = std::ffi::CString::new(format!("FnTable:{}", IVROVERLAY_VERSION)).unwrap();
        let mut iface_error: EVRInitError = 0;
        let table_ptr = unsafe { vr_get_generic_interface(version.as_ptr(), &mut iface_error) };
        if iface_error != 0 || table_ptr == 0 {
            return Err(format!(
                "IVROverlay インターフェース取得失敗: code={}",
                iface_error
            ));
        }

        // IVRApplications は自動起動登録(SteamVR起動時にこのexeを立ち上げてもらう)のためだけに
        // 使う補助インターフェース。取得できなくてもオーバーレイ本体には影響させない(non-fatal)。
        let applications_version =
            std::ffi::CString::new(format!("FnTable:{}", IVRAPPLICATIONS_VERSION)).unwrap();
        let mut applications_iface_error: EVRInitError = 0;
        let applications_table_ptr = unsafe {
            vr_get_generic_interface(applications_version.as_ptr(), &mut applications_iface_error)
        };
        let applications_table = if applications_iface_error != 0 || applications_table_ptr == 0 {
            eprintln!(
                "[SteamVR] IVRApplications インターフェース取得失敗 (自動起動登録は無効): code={}",
                applications_iface_error
            );
            None
        } else {
            Some(applications_table_ptr as *const VrIvApplicationsFnTable)
        };

        // IVRInput はボタン操作(SteamVR のバインド)を受け取るためだけに使う。
        // 取れなくてもオーバーレイ本体には影響させない(non-fatal)。
        let mut input_table = None;
        for v in IVRINPUT_VERSIONS {
            let name = std::ffi::CString::new(format!("FnTable:{}", v)).unwrap();
            let mut err: EVRInitError = 0;
            let ptr = unsafe { vr_get_generic_interface(name.as_ptr(), &mut err) };
            if err == 0 && ptr != 0 {
                input_table = Some(ptr as *const VrIvInputFnTable);
                break;
            }
            eprintln!("[SteamVR] {} インターフェース取得失敗: code={}", v, err);
        }

        Ok(Self {
            _lib: lib,
            overlay_table: table_ptr as *const VrIvOverlayFnTable,
            applications_table,
            input_table,
        })
    }

    pub fn overlay(&self) -> &VrIvOverlayFnTable {
        unsafe { &*self.overlay_table }
    }

    pub fn applications(&self) -> Option<&VrIvApplicationsFnTable> {
        self.applications_table.map(|ptr| unsafe { &*ptr })
    }

    pub fn input(&self) -> Option<&VrIvInputFnTable> {
        self.input_table.map(|ptr| unsafe { &*ptr })
    }
}

impl Drop for OpenVrLibrary {
    fn drop(&mut self) {
        let shutdown: Result<libloading::Symbol<unsafe extern "C" fn()>, _> =
            unsafe { self._lib.get(b"VR_ShutdownInternal\0") };
        if let Ok(vr_shutdown_internal) = shutdown {
            unsafe { vr_shutdown_internal() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fn_table_field_count_matches_official_header() {
        assert_eq!(
            std::mem::size_of::<VrIvOverlayFnTable>(),
            OVERLAY_FN_TABLE_FIELD_COUNT * std::mem::size_of::<usize>(),
            "VR_IVROverlay_FnTable のフィールド数が公式ヘッダと食い違っている可能性がある"
        );
    }

    #[test]
    fn test_applications_fn_table_field_count_matches_official_header() {
        assert_eq!(
            std::mem::size_of::<VrIvApplicationsFnTable>(),
            APPLICATIONS_FN_TABLE_FIELD_COUNT * std::mem::size_of::<usize>(),
            "VR_IVRApplications_FnTable のフィールド数が公式ヘッダと食い違っている可能性がある"
        );
    }

    #[test]
    fn test_input_fn_table_field_count_matches_official_header() {
        assert_eq!(
            std::mem::size_of::<VrIvInputFnTable>(),
            INPUT_FN_TABLE_FIELD_COUNT * std::mem::size_of::<usize>(),
            "VR_IVRInput_FnTable のフィールド数が公式ヘッダと食い違っている可能性がある"
        );
    }

    #[test]
    fn test_input_struct_sizes_match_official_header() {
        // VRActiveActionSet_t: u64×3 + u32 + i32 = 32
        assert_eq!(std::mem::size_of::<VRActiveActionSet_t>(), 32);
        // InputDigitalActionData_t: bool(+7) + u64 + bool + bool(+2) + f32 = 24
        assert_eq!(std::mem::size_of::<InputDigitalActionData_t>(), 24);
        // InputBindingInfo_t: 128×4 + 32 = 544
        assert_eq!(std::mem::size_of::<InputBindingInfo_t>(), 544);
    }

    #[test]
    fn test_texture_t_size_matches_official_header() {
        assert_eq!(
            std::mem::size_of::<Texture_t>(),
            16,
            "Texture_t のサイズが公式ヘッダ(16バイト)と食い違っている可能性がある"
        );
    }

    #[test]
    fn test_vrevent_t_size_matches_official_header() {
        assert_eq!(
            std::mem::size_of::<VREvent_t>(),
            64,
            "VREvent_t のサイズが公式ヘッダ(64バイト)と食い違っている可能性がある"
        );
    }

    #[test]
    fn test_hmd_vector2_t_size_matches_official_header() {
        assert_eq!(
            std::mem::size_of::<HmdVector2_t>(),
            8,
            "HmdVector2_t のサイズが公式ヘッダ(8バイト)と食い違っている可能性がある"
        );
    }
}
