pub mod bridge;
#[cfg(windows)]
mod dump;
#[cfg(windows)]
mod ffi;
#[cfg_attr(not(windows), allow(dead_code))]
mod input_manifest;
mod manifest;
#[cfg_attr(not(windows), allow(dead_code))]
mod overlay_ui;
#[cfg(windows)]
mod render;
#[cfg(windows)]
mod session;
mod settings_ui;
#[cfg_attr(not(windows), allow(dead_code))]
mod toast_ui;

pub use bridge::{OverlayAction, OverlayCommand, OverlayHandle, OverlaySnapshot, start};
#[cfg(windows)]
pub use dump::dump_overlay;
pub use settings_ui::{BindingButton, bindings_section};
