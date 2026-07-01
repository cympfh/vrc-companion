use enigo::{Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

pub fn call_qvpen() -> Result<(), String> {
    thread::spawn(|| {
        if let Err(e) = call_qvpen_sync() {
            eprintln!("call_qvpen failed: {}", e);
        }
    });
    Ok(())
}

#[cfg(windows)]
fn call_qvpen_sync() -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow};

    let title: Vec<u16> = "VRChat\0".encode_utf16().collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() {
        return Err("VRChat window not found".to_string());
    }
    unsafe { SetForegroundWindow(hwnd) };

    thread::sleep(Duration::from_millis(200));

    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("Failed to create Enigo: {:?}", e))?;

    enigo
        .key(Key::Tab, enigo::Direction::Press)
        .map_err(|e| format!("Failed to press Tab: {:?}", e))?;
    enigo
        .key(Key::Unicode('q'), enigo::Direction::Click)
        .map_err(|e| format!("Failed to press Q: {:?}", e))?;
    enigo
        .key(Key::Tab, enigo::Direction::Release)
        .map_err(|e| format!("Failed to release Tab: {:?}", e))?;

    Ok(())
}

#[cfg(not(windows))]
fn call_qvpen_sync() -> Result<(), String> {
    Err("call_qvpen is only supported on Windows".to_string())
}

/// アクティブなウィンドウにテキストを1文字ずつ入力する（非ブロッキング）
pub fn type_text(text: &str) -> Result<(), String> {
    let text_owned = text.to_string();
    thread::spawn(move || {
        if let Err(e) = type_text_sync(&text_owned) {
            eprintln!("Auto-input failed: {}", e);
        }
    });
    Ok(())
}

/// クリップボードの内容を Ctrl+V で貼り付ける（非ブロッキング）
pub fn send_ctrl_v() -> Result<(), String> {
    thread::spawn(move || {
        if let Err(e) = send_ctrl_v_sync() {
            eprintln!("Auto-input (Ctrl+V) failed: {}", e);
        }
    });
    Ok(())
}

/// Ctrl+V の後に Enter を送る（非ブロッキング）
pub fn send_ctrl_v_with_enter() -> Result<(), String> {
    thread::spawn(move || {
        if let Err(e) = send_ctrl_v_with_enter_sync() {
            eprintln!("Auto-input (Ctrl+V + Enter) failed: {}", e);
        }
    });
    Ok(())
}

/// テキストを入力した後に Enter を送る（非ブロッキング）
pub fn type_text_with_enter(text: &str) -> Result<(), String> {
    let text_owned = text.to_string();
    thread::spawn(move || {
        if let Err(e) = type_text_with_enter_sync(&text_owned) {
            eprintln!("Auto-input (typing + Enter) failed: {}", e);
        }
    });
    Ok(())
}

fn new_enigo() -> Result<Enigo, String> {
    Enigo::new(&Settings::default()).map_err(|e| format!("Failed to create Enigo: {:?}", e))
}

fn type_text_sync(text: &str) -> Result<(), String> {
    let mut enigo = new_enigo()?;
    thread::sleep(Duration::from_millis(100));
    enigo
        .text(text)
        .map_err(|e| format!("Failed to type text: {:?}", e))?;
    Ok(())
}

fn send_ctrl_v_sync() -> Result<(), String> {
    let mut enigo = new_enigo()?;
    thread::sleep(Duration::from_millis(100));
    enigo
        .key(Key::Control, enigo::Direction::Press)
        .map_err(|e| format!("Failed to press Ctrl: {:?}", e))?;
    enigo
        .key(Key::Unicode('v'), enigo::Direction::Click)
        .map_err(|e| format!("Failed to press V: {:?}", e))?;
    enigo
        .key(Key::Control, enigo::Direction::Release)
        .map_err(|e| format!("Failed to release Ctrl: {:?}", e))?;
    Ok(())
}

fn send_ctrl_v_with_enter_sync() -> Result<(), String> {
    send_ctrl_v_sync()?;
    thread::sleep(Duration::from_millis(100));
    press_enter()
}

fn type_text_with_enter_sync(text: &str) -> Result<(), String> {
    type_text_sync(text)?;
    thread::sleep(Duration::from_millis(100));
    press_enter()
}

fn press_enter() -> Result<(), String> {
    let mut enigo = new_enigo()?;
    enigo
        .key(Key::Return, enigo::Direction::Click)
        .map_err(|e| format!("Failed to press Enter: {:?}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(windows))]
    fn test_call_qvpen_unsupported_on_non_windows() {
        assert!(call_qvpen_sync().is_err());
    }
}
