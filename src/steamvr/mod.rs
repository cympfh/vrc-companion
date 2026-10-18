pub mod bridge;
#[cfg(windows)]
mod dump;
#[cfg(windows)]
mod ffi;
mod manifest;
#[cfg_attr(not(windows), allow(dead_code))]
mod overlay_ui;
#[cfg(windows)]
mod render;
#[cfg(windows)]
mod session;

pub use bridge::{OverlayAction, OverlayHandle, OverlaySnapshot, start};
#[cfg(windows)]
pub use dump::dump_overlay;
