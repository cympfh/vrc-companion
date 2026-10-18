//! SteamVR が動いているかの見張り。
//!
//! このアプリが SteamVR を起動してしまわないよう、`VR_Init` は vrserver.exe が動いているときだけ
//! 呼ぶ(SteamVR が動いていないときに Overlay として `VR_Init` すると、SteamVR が起動してしまう)。
//! 動いていなければ数秒おきに見に行き、SteamVR が来たらつなぎ直す。
//! 判定の部分は描画先に依存しないので全OSでテストできる。

use std::time::Duration;

/// SteamVR が動いている間だけ居るプロセス
pub const STEAMVR_SERVER_EXE: &str = "vrserver.exe";
/// SteamVR が動いていない・つなげなかったときに、もう一度見に行く間隔
pub const RETRY_EVERY: Duration = Duration::from_secs(5);
/// つながっている間に vrserver.exe が居なくなっていないかを見る間隔
pub const ALIVE_CHECK_EVERY: Duration = Duration::from_secs(1);

pub fn is_steamvr_server(process_name: &str) -> bool {
    process_name.eq_ignore_ascii_case(STEAMVR_SERVER_EXE)
}

/// つながっていた SteamVR との接続が終わった理由
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEnd {
    /// アプリ本体が閉じた(スレッドも終わる)
    AppClosed,
    /// SteamVR から終了の知らせ(VREvent_Quit)が来た
    SteamVrQuit,
    /// 知らせなしに vrserver.exe が居なくなった(落ちた・強制終了など)
    SteamVrGone,
    /// このアプリの別のコピーがもう SteamVR にタブを出している(例: 手で起動したあとに
    /// SteamVR の自動起動でもう1つ起動された)。このコピーは VR につなぐのをやめる
    AnotherInstance,
    /// つないだあとの準備(オーバーレイ作成など)に失敗した
    Failed(String),
}

impl SessionEnd {
    /// これで終わり(つなぎ直さない)か
    pub fn stops_retrying(&self) -> bool {
        matches!(self, SessionEnd::AppClosed | SessionEnd::AnotherInstance)
    }

    pub fn message(&self) -> String {
        match self {
            SessionEnd::AppClosed => "アプリを閉じる".to_string(),
            SessionEnd::AnotherInstance => {
                "このアプリの別のコピーがもう SteamVR につないでいるので、こちらは VR につながない"
                    .to_string()
            }
            SessionEnd::SteamVrQuit => {
                "SteamVR が終了したので切断した。SteamVR が起動したらつなぎ直す".to_string()
            }
            SessionEnd::SteamVrGone => {
                "SteamVR(vrserver.exe)が居なくなったので切断した。SteamVR が起動したらつなぎ直す"
                    .to_string()
            }
            SessionEnd::Failed(e) => format!(
                "準備に失敗したので切断した({}秒後にやり直す): {}",
                RETRY_EVERY.as_secs(),
                e
            ),
        }
    }
}

/// 同じ内容を何度もログに出さない(数秒おきにやり直すので)
#[derive(Debug, Default)]
pub struct StatusLog {
    last: Option<String>,
}

impl StatusLog {
    /// 前回と違うときだけ true(= ログに出す)
    pub fn changed(&mut self, message: &str) -> bool {
        if self.last.as_deref() == Some(message) {
            return false;
        }
        self.last = Some(message.to_string());
        true
    }

    pub fn log(&mut self, message: &str) {
        if self.changed(message) {
            eprintln!("[SteamVR] {}", message);
        }
    }
}

/// vrserver.exe が動いているか(プロセス一覧を見るだけで、SteamVR には触らない)
#[cfg(windows)]
pub fn steamvr_running() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut ok = Process32FirstW(snap, &mut entry) != 0;
        while ok {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            if is_steamvr_server(&String::from_utf16_lossy(&entry.szExeFile[..len])) {
                found = true;
                break;
            }
            ok = Process32NextW(snap, &mut entry) != 0;
        }
        CloseHandle(snap);
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_vrserver_counts_as_steamvr_running() {
        assert!(is_steamvr_server("vrserver.exe"));
        assert!(is_steamvr_server("VRServer.EXE"));
        // vrmonitor やスタートアップだけ残っていても、つなぎに行かない
        assert!(!is_steamvr_server("vrmonitor.exe"));
        assert!(!is_steamvr_server("vrstartup.exe"));
        assert!(!is_steamvr_server("vrserver.exe.bak"));
        assert!(!is_steamvr_server("vrc-companion.exe"));
    }

    #[test]
    fn status_log_reports_only_changes() {
        let mut log = StatusLog::default();
        assert!(log.changed("SteamVR の起動を待っています"));
        assert!(!log.changed("SteamVR の起動を待っています"));
        assert!(log.changed("接続した"));
        assert!(log.changed("SteamVR の起動を待っています"));
    }

    #[test]
    fn session_end_messages_say_what_happens_next() {
        assert!(SessionEnd::SteamVrQuit.message().contains("つなぎ直す"));
        assert!(SessionEnd::SteamVrGone.message().contains("つなぎ直す"));
        assert!(
            SessionEnd::Failed("x".into())
                .message()
                .contains("やり直す")
        );
    }

    #[test]
    fn only_app_exit_and_duplicate_stop_retrying() {
        assert!(SessionEnd::AppClosed.stops_retrying());
        assert!(SessionEnd::AnotherInstance.stops_retrying());
        assert!(!SessionEnd::SteamVrQuit.stops_retrying());
        assert!(!SessionEnd::SteamVrGone.stops_retrying());
        assert!(!SessionEnd::Failed("x".into()).stops_retrying());
    }

    #[test]
    fn retry_is_every_few_seconds() {
        assert!(RETRY_EVERY >= Duration::from_secs(2) && RETRY_EVERY <= Duration::from_secs(10));
        assert!(ALIVE_CHECK_EVERY < RETRY_EVERY);
    }
}
