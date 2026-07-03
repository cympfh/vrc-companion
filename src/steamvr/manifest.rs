//! SteamVR起動時にこのexeを自動起動してもらうためのOpenVRアプリケーションマニフェスト
//! (`.vrmanifest`)の組み立て・書き込み。
//!
//! 実際の`IVRApplications::AddApplicationManifest`/`SetApplicationAutoLaunch`呼び出しは
//! session.rs(Windows専用)側が担う。ここではファイル内容の組み立てとディスクへの
//! 書き込みのみを扱う純粋度の高いロジックであり、Windows専用のffi.rsとは独立させて
//! non-Windows host(WSL)でもユニットテストできるようにしている。

use std::path::PathBuf;

/// ダッシュボードオーバーレイの`CreateDashboardOverlay`キー(session.rsが`create_dashboard_overlay`
/// に渡す値)と一致させる必要がある — OpenVR仕様上、オーバーレイのキーと登録アプリの
/// app_keyが一致していないとSteamVR側で同一アプリとして関連付けられない。
pub const APP_KEY: &str = "cympfh.vrc_companion";
const APP_NAME: &str = "VRC Companion";
const MANIFEST_FILE_NAME: &str = "vrc-companion.vrmanifest";

/// OpenVRのアプリケーションマニフェストJSONを組み立てる純粋関数。
pub fn build_manifest_json(app_key: &str, name: &str, exe_path: &str) -> String {
    serde_json::json!({
        "source": "custom",
        "applications": [
            {
                "app_key": app_key,
                "launch_type": "binary",
                "binary_path_windows": exe_path,
                "is_dashboard_overlay": true,
                "strings": {
                    "en_us": {
                        "name": name
                    }
                }
            }
        ]
    })
    .to_string()
}

/// マニフェストを自身のconfig dir配下に書き込み、そのフルパスを返す。
/// session.rs(Windows専用)からのみ呼ばれるため、non-Windows host buildでは常にdead_code
/// になる — bridge.rsの`OverlayAction`と同じ理由で明示的に許容する。
#[allow(dead_code)]
pub fn write_manifest() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("current_exe取得失敗: {}", e))?;
    let json = build_manifest_json(APP_KEY, APP_NAME, &exe_path.to_string_lossy());

    let config_dir = dirs::config_dir()
        .ok_or_else(|| "config dir取得失敗".to_string())?
        .join("vrc-companion");
    std::fs::create_dir_all(&config_dir).map_err(|e| format!("config dir作成失敗: {}", e))?;

    let manifest_path = config_dir.join(MANIFEST_FILE_NAME);
    std::fs::write(&manifest_path, json).map_err(|e| format!("マニフェスト書き込み失敗: {}", e))?;

    Ok(manifest_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_manifest_json_contains_app_key_name_and_path() {
        let json = build_manifest_json("test.app_key", "Test App", r"C:\path\to\app.exe");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let app = &parsed["applications"][0];
        assert_eq!(app["app_key"], "test.app_key");
        assert_eq!(app["launch_type"], "binary");
        assert_eq!(app["binary_path_windows"], r"C:\path\to\app.exe");
        assert_eq!(app["is_dashboard_overlay"], true);
        assert_eq!(app["strings"]["en_us"]["name"], "Test App");
    }

    #[test]
    fn test_write_manifest_writes_file_with_own_app_key() {
        let path = write_manifest().unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["applications"][0]["app_key"], APP_KEY);
        std::fs::remove_file(&path).ok();
    }
}
