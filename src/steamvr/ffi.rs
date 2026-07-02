//! OpenVR プレーンC API (openvr_api.dll) への生FFIバインディング。
//!
//! 公式OpenVR C++ SDKはMSVCビルドでvtable ABIがこのプロジェクトのクロスコンパイル先
//! (x86_64-pc-windows-gnu) と非互換なため使えない。代わりに `libloading` で
//! `openvr_api.dll` を動的ロードし、`VR_GetGenericInterface("FnTable:IVROverlay_028", ...)`
//! で ABI安全な関数ポインタテーブルを取得する。
//!
//! フィールドの型・順序は公式ヘッダ
//! https://github.com/ValveSoftware/openvr/blob/master/headers/openvr_capi.h
//! (取得日 2026-07-01) から逐語的に転記した。順序はABIクリティカル — 変更しないこと。
//! 実際に呼び出さないフィールドは、正しい順序・サイズを保つためのプレースホルダ
//! (`PlaceholderFn`) にしている。

use libloading::Library;
use std::ffi::{c_char, c_void};

pub const IVROVERLAY_VERSION: &str = "IVROverlay_028";
const VR_APPLICATION_TYPE_OVERLAY: i32 = 2;

pub type VrOverlayHandle = u64;
pub type EVRInitError = i32;
pub type EVROverlayError = i32;

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
    pub set_overlay_width_in_meters: PlaceholderFn,
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
    pub wait_frame_sync: PlaceholderFn,
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
    pub trigger_laser_mouse_haptic_vibration: PlaceholderFn,
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

/// openvr_api.dll をロードし、IVROverlay の関数ポインタテーブルを保持する。
/// `_lib` が生きている間だけ `overlay_table` は有効。
pub struct OpenVrLibrary {
    _lib: Library,
    overlay_table: *const VrIvOverlayFnTable,
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

        Ok(Self {
            _lib: lib,
            overlay_table: table_ptr as *const VrIvOverlayFnTable,
        })
    }

    pub fn overlay(&self) -> &VrIvOverlayFnTable {
        unsafe { &*self.overlay_table }
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
