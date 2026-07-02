use std::sync::mpsc::{Receiver, Sender};

use crate::config::Config;

/// デスクトップ GUI 側の Config から VR オーバーレイ描画スレッドへ送るスナップショット。
#[derive(Debug, Clone, PartialEq)]
pub struct OverlaySnapshot {
    pub clipboard_enabled: bool,
    pub auto_input_enabled: bool,
    pub auto_input_send_enter: bool,
    pub vrchat_enabled: bool,
    pub eliza_enabled: bool,
    pub eliza_response_to_vrchat_enabled: bool,
    pub auto_translate_enabled: bool,
    pub is_recording: bool,
    pub last_transcription: String,
    pub last_eliza_response: String,
    pub last_translated_response: String,
}

impl OverlaySnapshot {
    pub fn from_config(
        config: &Config,
        is_recording: bool,
        last_transcription: &str,
        last_eliza_response: &str,
        last_translated_response: &str,
    ) -> Self {
        Self {
            clipboard_enabled: config.clipboard_enabled,
            auto_input_enabled: config.auto_input_enabled,
            auto_input_send_enter: config.auto_input_send_enter,
            vrchat_enabled: config.vrchat_enabled,
            eliza_enabled: config.eliza_enabled,
            eliza_response_to_vrchat_enabled: config.eliza_response_to_vrchat_enabled,
            auto_translate_enabled: config.auto_translate_enabled,
            is_recording,
            last_transcription: last_transcription.to_string(),
            last_eliza_response: last_eliza_response.to_string(),
            last_translated_response: last_translated_response.to_string(),
        }
    }
}

/// VR オーバーレイに描画する1行分の情報。デスクトップGUI(main.rs)のチェックボックスと
/// 同じラベル・順序・インデント構造を保つ。
pub struct OverlayField {
    pub label: &'static str,
    pub enabled: bool,
    pub indent: bool,
    pub action: OverlayAction,
}

/// デスクトップGUIのチェックボックス群と対応するオーバーレイ表示用フィールド一覧を返す。
/// 純粋関数なので全OSでテスト可能。
pub fn overlay_fields(snapshot: &OverlaySnapshot) -> Vec<OverlayField> {
    vec![
        OverlayField {
            label: "Copy to clipboard",
            enabled: snapshot.clipboard_enabled,
            indent: false,
            action: OverlayAction::ToggleClipboard,
        },
        OverlayField {
            label: "Input to active window",
            enabled: snapshot.auto_input_enabled,
            indent: false,
            action: OverlayAction::ToggleAutoInput,
        },
        OverlayField {
            label: "Send Enter after input",
            enabled: snapshot.auto_input_send_enter,
            indent: true,
            action: OverlayAction::ToggleAutoInputSendEnter,
        },
        OverlayField {
            label: "Input to VRChat",
            enabled: snapshot.vrchat_enabled,
            indent: false,
            action: OverlayAction::ToggleVrchat,
        },
        OverlayField {
            label: "自動翻訳 by Eliza",
            enabled: snapshot.auto_translate_enabled,
            indent: false,
            action: OverlayAction::ToggleAutoTranslate,
        },
        OverlayField {
            label: "お話 with Eliza",
            enabled: snapshot.eliza_enabled,
            indent: false,
            action: OverlayAction::ToggleEliza,
        },
        OverlayField {
            label: "Send Eliza's response to VRChat",
            enabled: snapshot.eliza_response_to_vrchat_enabled,
            indent: true,
            action: OverlayAction::ToggleElizaResponseToVrchat,
        },
    ]
}

/// VR オーバーレイ側のチェックボックス操作 → デスクトップ側 Config への反映アクション。
/// 各バリアントは main.rs の apply_steamvr_action で match され、
/// session.rs の PollNextOverlayEvent 配線(render.rsのクリック検出経由)から construct される。
/// その実際の construct 元である `overlay_fields`/render.rs は、非Windows host build では
/// (テスト以外)呼び出されない(render.rs自体が`#[cfg(windows)]`)ため、dead_code は
/// 常にこのプラットフォームで発生する — enumレベルで明示的に許容する。
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayAction {
    ToggleClipboard,
    ToggleAutoInput,
    ToggleAutoInputSendEnter,
    ToggleVrchat,
    ToggleEliza,
    ToggleElizaResponseToVrchat,
    ToggleAutoTranslate,
    CallQvPen,
    ToggleRecording,
}

pub struct OverlayHandle {
    pub snapshot_tx: Sender<OverlaySnapshot>,
    pub action_rx: Receiver<OverlayAction>,
}

/// SteamVR オーバーレイ描画スレッドを起動する。SteamVR未起動・DLL不在などは
/// 非致命的に扱い None を返す（呼び出し側は他機能に影響させない）。
#[cfg(windows)]
pub fn start(initial: OverlaySnapshot) -> Option<OverlayHandle> {
    super::session::start(initial)
}

#[cfg(not(windows))]
pub fn start(_initial: OverlaySnapshot) -> Option<OverlayHandle> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_snapshot_from_config_copies_all_fields() {
        let mut config = Config::default();
        config.clipboard_enabled = false;
        config.auto_input_enabled = true;
        config.auto_input_send_enter = true;
        config.vrchat_enabled = false;
        config.eliza_enabled = true;
        config.eliza_response_to_vrchat_enabled = true;
        config.auto_translate_enabled = false;

        let snapshot =
            OverlaySnapshot::from_config(&config, true, "こんにちは", "Eliza応答", "translated");

        assert_eq!(snapshot.clipboard_enabled, config.clipboard_enabled);
        assert_eq!(snapshot.auto_input_enabled, config.auto_input_enabled);
        assert_eq!(snapshot.auto_input_send_enter, config.auto_input_send_enter);
        assert_eq!(snapshot.vrchat_enabled, config.vrchat_enabled);
        assert_eq!(snapshot.eliza_enabled, config.eliza_enabled);
        assert_eq!(
            snapshot.eliza_response_to_vrchat_enabled,
            config.eliza_response_to_vrchat_enabled
        );
        assert_eq!(
            snapshot.auto_translate_enabled,
            config.auto_translate_enabled
        );
        assert!(snapshot.is_recording);
        assert_eq!(snapshot.last_transcription, "こんにちは");
        assert_eq!(snapshot.last_eliza_response, "Eliza応答");
        assert_eq!(snapshot.last_translated_response, "translated");
    }

    #[test]
    fn test_start_returns_none_or_some_without_panicking() {
        // 実機(SteamVR)無しでも呼べることだけを保証する。非Windowsでは常にNone。
        let snapshot = OverlaySnapshot::from_config(&Config::default(), false, "", "", "");
        let _ = start(snapshot);
    }

    fn all_false_snapshot() -> OverlaySnapshot {
        OverlaySnapshot {
            clipboard_enabled: false,
            auto_input_enabled: false,
            auto_input_send_enter: false,
            vrchat_enabled: false,
            eliza_enabled: false,
            eliza_response_to_vrchat_enabled: false,
            auto_translate_enabled: false,
            is_recording: false,
            last_transcription: String::new(),
            last_eliza_response: String::new(),
            last_translated_response: String::new(),
        }
    }

    #[test]
    fn test_overlay_fields_labels_order_and_indent() {
        let fields = overlay_fields(&all_false_snapshot());
        let expected: Vec<(&str, bool, OverlayAction)> = vec![
            ("Copy to clipboard", false, OverlayAction::ToggleClipboard),
            (
                "Input to active window",
                false,
                OverlayAction::ToggleAutoInput,
            ),
            (
                "Send Enter after input",
                true,
                OverlayAction::ToggleAutoInputSendEnter,
            ),
            ("Input to VRChat", false, OverlayAction::ToggleVrchat),
            (
                "自動翻訳 by Eliza",
                false,
                OverlayAction::ToggleAutoTranslate,
            ),
            ("お話 with Eliza", false, OverlayAction::ToggleEliza),
            (
                "Send Eliza's response to VRChat",
                true,
                OverlayAction::ToggleElizaResponseToVrchat,
            ),
        ];
        assert_eq!(fields.len(), expected.len());
        for (field, (label, indent, action)) in fields.iter().zip(expected.iter()) {
            assert_eq!(field.label, *label);
            assert_eq!(field.indent, *indent);
            assert_eq!(field.action, *action);
        }
    }

    #[test]
    fn test_overlay_fields_reflects_each_snapshot_flag_independently() {
        let setters: Vec<(fn(&mut OverlaySnapshot), usize)> = vec![
            (|s| s.clipboard_enabled = true, 0),
            (|s| s.auto_input_enabled = true, 1),
            (|s| s.auto_input_send_enter = true, 2),
            (|s| s.vrchat_enabled = true, 3),
            (|s| s.auto_translate_enabled = true, 4),
            (|s| s.eliza_enabled = true, 5),
            (|s| s.eliza_response_to_vrchat_enabled = true, 6),
        ];

        for (set_flag, expected_index) in setters {
            let mut snapshot = all_false_snapshot();
            set_flag(&mut snapshot);
            let fields = overlay_fields(&snapshot);
            for (i, field) in fields.iter().enumerate() {
                assert_eq!(
                    field.enabled,
                    i == expected_index,
                    "index {} の enabled が意図しない値: {:?}",
                    i,
                    field.label
                );
            }
        }
    }
}
