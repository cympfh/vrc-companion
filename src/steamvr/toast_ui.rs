//! 録音の開始/終了を知らせるトースト(小さな別オーバーレイ)。
//!
//! ダッシュボードのタブとは別の、頭に固定したオーバーレイに出すので、ダッシュボードを
//! 閉じて VRChat を遊んでいる最中でも見える。表示は約 2 秒で、ふわっと出てふわっと消える
//! (透明度は SteamVR の SetOverlayAlpha で変える)。描画先に依存しないので全OSでテストできる。

use std::time::Duration;

use egui::{Color32, FontId, Rect, Rounding, Stroke, pos2, vec2};

use super::overlay_ui::color;

/// トーストの論理サイズ(テクスチャはこの PIXELS_PER_POINT 倍)
pub const TOAST_W: f32 = 460.0;
pub const TOAST_H: f32 = 120.0;
/// VR の中での幅(メートル)と、目の前のどこに出すか(頭から見て、少し下・1m 先)
pub const TOAST_WIDTH_M: f32 = 0.42;
pub const TOAST_OFFSET_M: [f32; 3] = [0.0, -0.22, -1.0];

const FADE_IN: Duration = Duration::from_millis(150);
const HOLD_UNTIL: Duration = Duration::from_millis(1700);
const GONE_AT: Duration = Duration::from_millis(2200);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    RecordingStarted,
    RecordingStopped,
}

impl ToastKind {
    /// 録音中かどうかが変わったら、出すトーストを返す
    pub fn from_transition(was_recording: bool, is_recording: bool) -> Option<Self> {
        match (was_recording, is_recording) {
            (false, true) => Some(ToastKind::RecordingStarted),
            (true, false) => Some(ToastKind::RecordingStopped),
            _ => None,
        }
    }

    fn title(self) -> &'static str {
        match self {
            ToastKind::RecordingStarted => "録音開始",
            ToastKind::RecordingStopped => "録音終了",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            ToastKind::RecordingStarted => "話しかけてください",
            ToastKind::RecordingStopped => "録音を止めました",
        }
    }
}

/// 出してからの経過時間 → 透明度(1 = くっきり)。消えたら None
pub fn alpha_at(elapsed: Duration) -> Option<f32> {
    if elapsed >= GONE_AT {
        None
    } else if elapsed < FADE_IN {
        Some(elapsed.as_secs_f32() / FADE_IN.as_secs_f32())
    } else if elapsed <= HOLD_UNTIL {
        Some(1.0)
    } else {
        let t = (elapsed - HOLD_UNTIL).as_secs_f32() / (GONE_AT - HOLD_UNTIL).as_secs_f32();
        Some(1.0 - t)
    }
}

/// トースト1枚を描く(背景は透明、角丸のカード)
pub fn draw(ctx: &egui::Context, kind: ToastKind) {
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(Color32::TRANSPARENT))
        .show(ctx, |ui| {
            let full = ui.max_rect();
            let rect = full.shrink(4.0);
            let p = ui.painter();
            p.rect(
                rect,
                Rounding::same(rect.height() / 2.0),
                Color32::from_rgba_unmultiplied(0x16, 0x19, 0x20, 240),
                Stroke::new(1.5, color::CARD_BORDER),
            );
            // 左の丸いアイコン: 開始は赤い丸(録音ボタン)、終了は灰色の四角(停止)
            let c = pos2(rect.left() + rect.height() / 2.0, rect.center().y);
            let (accent, ring) = match kind {
                ToastKind::RecordingStarted => (
                    color::RECORD,
                    Color32::from_rgba_unmultiplied(0xe5, 0x48, 0x4d, 60),
                ),
                ToastKind::RecordingStopped => (color::SUBTEXT, color::SECONDARY),
            };
            p.circle_filled(c, 36.0, ring);
            match kind {
                ToastKind::RecordingStarted => {
                    p.circle_filled(c, 18.0, accent);
                }
                ToastKind::RecordingStopped => {
                    p.rect_filled(
                        Rect::from_center_size(c, vec2(30.0, 30.0)),
                        Rounding::same(6.0),
                        accent,
                    );
                }
            }
            let x = rect.left() + rect.height() + 4.0;
            p.text(
                pos2(x, rect.center().y - 16.0),
                egui::Align2::LEFT_CENTER,
                kind.title(),
                FontId::proportional(36.0),
                color::TEXT,
            );
            p.text(
                pos2(x + 2.0, rect.center().y + 24.0),
                egui::Align2::LEFT_CENTER,
                kind.subtitle(),
                FontId::proportional(18.0),
                color::SUBTEXT,
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_picks_the_right_toast() {
        assert_eq!(
            ToastKind::from_transition(false, true),
            Some(ToastKind::RecordingStarted)
        );
        assert_eq!(
            ToastKind::from_transition(true, false),
            Some(ToastKind::RecordingStopped)
        );
        assert_eq!(ToastKind::from_transition(true, true), None);
        assert_eq!(ToastKind::from_transition(false, false), None);
    }

    #[test]
    fn fades_in_holds_and_fades_out_in_about_two_seconds() {
        let ms = Duration::from_millis;
        assert_eq!(alpha_at(ms(0)), Some(0.0));
        assert!(alpha_at(ms(75)).unwrap() > 0.4);
        assert_eq!(alpha_at(ms(1000)), Some(1.0));
        let fading = alpha_at(ms(1950)).unwrap();
        assert!(fading > 0.0 && fading < 1.0);
        assert_eq!(alpha_at(ms(2200)), None);
        assert_eq!(alpha_at(ms(5000)), None);
    }

    #[test]
    fn text_fits_inside_the_toast() {
        let ctx = egui::Context::default();
        for kind in [ToastKind::RecordingStarted, ToastKind::RecordingStopped] {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(TOAST_W, TOAST_H),
                )),
                ..Default::default()
            };
            let out = ctx.run(input, |ctx| draw(ctx, kind));
            for shape in out.shapes {
                let r = shape.shape.visual_bounding_rect();
                if r.is_positive() {
                    assert!(
                        r.right() <= TOAST_W && r.bottom() <= TOAST_H && r.left() >= 0.0,
                        "{:?} がはみ出している: {:?}",
                        kind,
                        r
                    );
                }
            }
        }
    }
}
