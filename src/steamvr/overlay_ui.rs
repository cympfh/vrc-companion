//! SteamVR オーバーレイの中身(egui)。描画先(WGL / テスト)に依存しないので全OSでテストできる。
//!
//! 見た目: 暗い背景にカードを並べる。上から、見出しと録音中の表示 → Start/Stop と QvPen の大きなボタン →
//! 送り先・Eliza の2枚のカード(各行は右端にスイッチ) → ボタン割り当て(SteamVR)のカード →
//! 認識結果のカード。
//! 座標はすべて論理ピクセル(`LOGICAL_W` × `LOGICAL_H`)。実際のテクスチャは
//! `PIXELS_PER_POINT` 倍の解像度で描くので、大きく表示しても文字がぼやけない。

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Color32, FontId, Pos2, Rect, Rounding, Sense, Stroke, pos2, vec2};

use super::bridge::{
    InputStatus, InputView, OverlayAction, OverlayField, OverlaySnapshot, overlay_fields,
};

/// UI の論理サイズ(マウス座標もこの単位)
pub const LOGICAL_W: f32 = 600.0;
pub const LOGICAL_H: f32 = 790.0;
/// テクスチャの解像度 = 論理サイズ × これ
pub const PIXELS_PER_POINT: f32 = 2.0;

pub(super) mod color {
    use egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(0x12, 0x14, 0x19);
    pub const CARD: Color32 = Color32::from_rgb(0x1b, 0x1e, 0x26);
    pub const CARD_BORDER: Color32 = Color32::from_rgb(0x2a, 0x2e, 0x38);
    pub const TEXT: Color32 = Color32::from_rgb(0xee, 0xf0, 0xf4);
    pub const SUBTEXT: Color32 = Color32::from_rgb(0x9a, 0xa1, 0xad);
    pub const CAPTION: Color32 = Color32::from_rgb(0x7c, 0x84, 0x92);
    pub const ACCENT: Color32 = Color32::from_rgb(0x4c, 0x8d, 0xf6);
    pub const RECORD: Color32 = Color32::from_rgb(0xe5, 0x48, 0x4d);
    pub const TRACK_OFF: Color32 = Color32::from_rgb(0x3a, 0x3f, 0x4b);
    pub const SECONDARY: Color32 = Color32::from_rgb(0x2a, 0x2f, 0x3a);
    pub const FIRED: Color32 = Color32::from_rgb(0x3d, 0xd6, 0x8c);
}

const PAD: f32 = 20.0;
const GAP: f32 = 14.0;
const RADIUS: f32 = 14.0;

/// 押せる場所1つ分(テストでクリック位置を決めるのにも使う)
#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct HitTarget {
    pub action: OverlayAction,
    pub rect: Rect,
}

/// 見た目の設定(フォント以外)。新しい egui::Context ごとに1回呼ぶ。
pub fn setup_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = color::BG;
    style.visuals.override_text_color = Some(color::TEXT);
    style.interaction.selectable_labels = false;
    ctx.set_style(style);
}

/// 1フレーム分の UI を描き、押されたアクションと押せる場所の一覧を返す。
pub fn draw(
    ctx: &egui::Context,
    snapshot: &OverlaySnapshot,
) -> (Vec<OverlayAction>, Vec<HitTarget>) {
    let mut clicked = Vec::new();
    let mut targets = Vec::new();
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(color::BG))
        .show(ctx, |ui| {
            let full = ui.max_rect();
            let mut y = full.top() + PAD;
            let left = full.left() + PAD;
            let width = full.width() - PAD * 2.0;

            header(
                ui,
                snapshot,
                Rect::from_min_size(pos2(left, y), vec2(width, 40.0)),
            );
            y += 40.0 + GAP;

            // 大きなボタン2つ
            let qv_w = 168.0;
            let rec_rect = Rect::from_min_size(pos2(left, y), vec2(width - qv_w - GAP, 64.0));
            let (rec_label, rec_fill) = if snapshot.is_recording {
                ("⏹ Stop", color::RECORD)
            } else {
                ("⏺ Start", color::ACCENT)
            };
            if big_button(ui, "rec", rec_rect, rec_label, rec_fill, Color32::WHITE) {
                clicked.push(OverlayAction::ToggleRecording);
            }
            targets.push(HitTarget {
                action: OverlayAction::ToggleRecording,
                rect: rec_rect,
            });
            let qv_rect = Rect::from_min_size(pos2(rec_rect.right() + GAP, y), vec2(qv_w, 64.0));
            if big_button(
                ui,
                "qvpen",
                qv_rect,
                "📝 call QvPen",
                color::SECONDARY,
                color::TEXT,
            ) {
                clicked.push(OverlayAction::CallQvPen);
            }
            targets.push(HitTarget {
                action: OverlayAction::CallQvPen,
                rect: qv_rect,
            });
            y += 64.0 + GAP;

            // スイッチのカード: 送り先(先頭4つ)と Eliza(残り)
            let fields = overlay_fields(snapshot);
            let (outputs, eliza) = fields.split_at(4);
            let col_w = (width - GAP) / 2.0;
            let h1 = toggle_card(
                ui,
                "送り先",
                outputs,
                pos2(left, y),
                col_w,
                &mut clicked,
                &mut targets,
            );
            let h2 = toggle_card(
                ui,
                "Eliza",
                eliza,
                pos2(left + col_w + GAP, y),
                col_w,
                &mut clicked,
                &mut targets,
            );
            y += h1.max(h2) + GAP;

            // ボタン割り当て(SteamVR)
            let h3 = binding_card(
                ui,
                &snapshot.input,
                pos2(left, y),
                width,
                &mut clicked,
                &mut targets,
            );
            y += h3 + GAP;

            // 認識結果
            let text_rect =
                Rect::from_min_max(pos2(left, y), pos2(left + width, full.bottom() - PAD));
            text_card(ui, snapshot, text_rect);
        });
    (clicked, targets)
}

fn header(ui: &egui::Ui, snapshot: &OverlaySnapshot, rect: Rect) {
    let p = ui.painter();
    p.text(
        pos2(rect.left() + 2.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "VRC Companion",
        FontId::proportional(26.0),
        color::TEXT,
    );
    // 右端: 録音中 / 待機中
    let (label, dot, bg) = if snapshot.is_recording {
        (
            "録音中",
            color::RECORD,
            Color32::from_rgba_unmultiplied(0xe5, 0x48, 0x4d, 48),
        )
    } else {
        ("待機中", color::SUBTEXT, color::SECONDARY)
    };
    let galley =
        ui.fonts(|f| f.layout_no_wrap(label.to_string(), FontId::proportional(16.0), color::TEXT));
    let pill_w = galley.size().x + 44.0;
    let pill = Rect::from_center_size(
        pos2(rect.right() - pill_w / 2.0, rect.center().y),
        vec2(pill_w, 32.0),
    );
    p.rect_filled(pill, Rounding::same(16.0), bg);
    p.circle_filled(pos2(pill.left() + 18.0, pill.center().y), 5.0, dot);
    p.galley(
        pos2(pill.left() + 30.0, pill.center().y - galley.size().y / 2.0),
        galley,
        color::TEXT,
    );
}

/// 押されたら true。押している間・指している間は色が変わる。
fn big_button(
    ui: &mut egui::Ui,
    id: &str,
    rect: Rect,
    label: &str,
    fill: Color32,
    text: Color32,
) -> bool {
    let resp = ui.interact(rect, ui.id().with(id), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, Rounding::same(RADIUS), fill);
    if resp.is_pointer_button_down_on() {
        p.rect_filled(rect, Rounding::same(RADIUS), Color32::from_black_alpha(60));
    } else if resp.hovered() {
        p.rect_filled(rect, Rounding::same(RADIUS), Color32::from_white_alpha(22));
    }
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(22.0),
        text,
    );
    resp.clicked()
}

fn card(ui: &egui::Ui, rect: Rect) {
    ui.painter().rect(
        rect,
        Rounding::same(RADIUS),
        color::CARD,
        Stroke::new(1.0, color::CARD_BORDER),
    );
}

/// スイッチの並んだカード。高さを返す。
fn toggle_card(
    ui: &mut egui::Ui,
    title: &str,
    fields: &[OverlayField],
    min: Pos2,
    width: f32,
    clicked: &mut Vec<OverlayAction>,
    targets: &mut Vec<HitTarget>,
) -> f32 {
    let inner = 8.0;
    let title_h = 34.0;
    // 先に各行の高さを決める(長いラベルは折り返す)
    let rows: Vec<(f32, std::sync::Arc<egui::Galley>)> = fields
        .iter()
        .map(|f| {
            let indent = if f.indent { 22.0 } else { 0.0 };
            let size = if f.indent { 14.0 } else { 16.0 };
            let wrap = width - inner * 2.0 - 12.0 - indent - 44.0 - 20.0;
            let color = if f.indent {
                color::SUBTEXT
            } else {
                color::TEXT
            };
            let g = ui.fonts(|fo| {
                fo.layout(f.label.to_string(), FontId::proportional(size), color, wrap)
            });
            ((g.size().y + 20.0).max(46.0), g)
        })
        .collect();
    let height = title_h + rows.iter().map(|r| r.0).sum::<f32>() + inner * 2.0;
    let rect = Rect::from_min_size(min, vec2(width, height));
    card(ui, rect);
    ui.painter().text(
        pos2(rect.left() + 20.0, rect.top() + 24.0),
        egui::Align2::LEFT_CENTER,
        title,
        FontId::proportional(14.0),
        color::CAPTION,
    );
    let mut y = rect.top() + title_h + inner;
    for (field, (h, galley)) in fields.iter().zip(rows) {
        let row = Rect::from_min_size(pos2(rect.left() + inner, y), vec2(width - inner * 2.0, h));
        let resp = ui.interact(row, ui.id().with(field.label), Sense::click());
        let p = ui.painter();
        if resp.is_pointer_button_down_on() {
            p.rect_filled(row, Rounding::same(10.0), Color32::from_white_alpha(20));
        } else if resp.hovered() {
            p.rect_filled(row, Rounding::same(10.0), Color32::from_white_alpha(10));
        }
        let indent = if field.indent { 22.0 } else { 0.0 };
        if field.indent {
            // 親の項目にぶら下がっていることを示す細い線
            p.line_segment(
                [
                    pos2(row.left() + 18.0, row.top() + 6.0),
                    pos2(row.left() + 18.0, row.bottom() - 6.0),
                ],
                Stroke::new(2.0, color::CARD_BORDER),
            );
        }
        p.galley(
            pos2(
                row.left() + 12.0 + indent,
                row.center().y - galley.size().y / 2.0,
            ),
            galley,
            color::TEXT,
        );
        let on =
            ui.ctx()
                .animate_bool_with_time(ui.id().with(("sw", field.label)), field.enabled, 0.12);
        switch(
            ui,
            pos2(row.right() - 12.0 - 44.0, row.center().y - 13.0),
            on,
        );
        if resp.clicked() {
            clicked.push(field.action);
        }
        targets.push(HitTarget {
            action: field.action,
            rect: row,
        });
        y += h;
    }
    height
}

/// SteamVR のボタン割り当てのカード。操作ごとに、いま割り当てられているボタンを出す。
/// 押されると少しの間「反応」が点く(動作確認用)。右上に割り当て画面を開くボタン。高さを返す。
fn binding_card(
    ui: &mut egui::Ui,
    view: &InputView,
    min: Pos2,
    width: f32,
    clicked: &mut Vec<OverlayAction>,
    targets: &mut Vec<HitTarget>,
) -> f32 {
    let title_h = 60.0;
    let row_h = 30.0;
    let rows = match view.status {
        InputStatus::Ready => view.actions.len().max(1),
        _ => 1,
    };
    let height = title_h + row_h * rows as f32 + 14.0;
    let rect = Rect::from_min_size(min, vec2(width, height));
    card(ui, rect);
    ui.painter().text(
        pos2(rect.left() + 20.0, rect.top() + 30.0),
        egui::Align2::LEFT_CENTER,
        "ボタン割り当て（SteamVR）",
        FontId::proportional(14.0),
        color::CAPTION,
    );
    let btn = Rect::from_min_size(
        pos2(rect.right() - 8.0 - 200.0, rect.top() + 8.0),
        vec2(200.0, 44.0),
    );
    if small_button(ui, "open_binding_ui", btn, "割り当てを変える") {
        clicked.push(OverlayAction::OpenBindingUi);
    }
    targets.push(HitTarget {
        action: OverlayAction::OpenBindingUi,
        rect: btn,
    });

    let p = ui.painter_at(rect.shrink(2.0));
    let mut y = rect.top() + title_h;
    let label_x = rect.left() + 20.0;
    let value_x = rect.left() + 190.0;
    let message = match &view.status {
        InputStatus::Starting => Some("SteamVR に接続中…".to_string()),
        InputStatus::Unavailable(why) => Some(format!("使えません: {}", why)),
        InputStatus::Ready if view.actions.is_empty() => Some("操作がありません".to_string()),
        InputStatus::Ready => None,
    };
    if let Some(m) = message {
        p.text(
            pos2(label_x, y + row_h / 2.0),
            egui::Align2::LEFT_CENTER,
            m,
            FontId::proportional(15.0),
            color::SUBTEXT,
        );
        return height;
    }
    for a in &view.actions {
        let cy = y + row_h / 2.0;
        p.text(
            pos2(label_x, cy),
            egui::Align2::LEFT_CENTER,
            a.label,
            FontId::proportional(16.0),
            color::TEXT,
        );
        let (value, col) = if a.bindings.is_empty() {
            ("未割り当て".to_string(), color::CAPTION)
        } else {
            (a.bindings.join(" / "), color::SUBTEXT)
        };
        let fired_w = if a.fired { 64.0 } else { 0.0 };
        let mut job = LayoutJob::single_section(
            value,
            TextFormat {
                font_id: FontId::proportional(15.0),
                color: col,
                ..Default::default()
            },
        );
        job.wrap = TextWrapping {
            max_width: rect.right() - 20.0 - fired_w - value_x,
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        };
        let g = ui.fonts(|f| f.layout_job(job));
        p.galley(pos2(value_x, cy - g.size().y / 2.0), g, col);
        if a.fired {
            let pill =
                Rect::from_center_size(pos2(rect.right() - 20.0 - 28.0, cy), vec2(56.0, 24.0));
            p.rect_filled(
                pill,
                Rounding::same(12.0),
                Color32::from_rgba_unmultiplied(0x3d, 0xd6, 0x8c, 48),
            );
            p.text(
                pill.center(),
                egui::Align2::CENTER_CENTER,
                "反応",
                FontId::proportional(14.0),
                color::FIRED,
            );
        }
        y += row_h;
    }
    height
}

/// カードの中の小さめのボタン。押されたら true。
fn small_button(ui: &mut egui::Ui, id: &str, rect: Rect, label: &str) -> bool {
    let resp = ui.interact(rect, ui.id().with(id), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, Rounding::same(10.0), color::SECONDARY);
    if resp.is_pointer_button_down_on() {
        p.rect_filled(rect, Rounding::same(10.0), Color32::from_black_alpha(60));
    } else if resp.hovered() {
        p.rect_filled(rect, Rounding::same(10.0), Color32::from_white_alpha(22));
    }
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(16.0),
        color::TEXT,
    );
    resp.clicked()
}

/// 44×26 のスイッチ。`t` は 0(オフ)〜1(オン)。
fn switch(ui: &egui::Ui, min: Pos2, t: f32) {
    let rect = Rect::from_min_size(min, vec2(44.0, 26.0));
    let p = ui.painter();
    let track = lerp_color(color::TRACK_OFF, color::ACCENT, t);
    p.rect_filled(rect, Rounding::same(13.0), track);
    let x = egui::lerp(rect.left() + 13.0..=rect.right() - 13.0, t);
    p.circle_filled(pos2(x, rect.center().y), 10.0, Color32::WHITE);
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// 最後の認識結果と、Eliza の返事 / 翻訳結果(今までと同じ出し分け)
fn text_card(ui: &egui::Ui, snapshot: &OverlaySnapshot, rect: Rect) {
    if rect.height() < 40.0 {
        return;
    }
    card(ui, rect);
    // (見出し, 本文, 最大の行数)。入りきらない分は「…」で切る
    let mut sections: Vec<(&str, &str, usize)> =
        vec![("Transcribed", snapshot.last_transcription.as_str(), 2)];
    if snapshot.eliza_enabled && !snapshot.last_eliza_response.is_empty() {
        sections.push(("Eliza", snapshot.last_eliza_response.as_str(), 3));
    } else if snapshot.auto_translate_enabled && !snapshot.last_translated_response.is_empty() {
        sections.push(("翻訳結果", snapshot.last_translated_response.as_str(), 3));
    }
    let p = ui.painter_at(rect.shrink(2.0));
    let wrap = rect.width() - 40.0;
    let mut y = rect.top() + 14.0;
    for (caption, body, max_rows) in sections {
        p.text(
            pos2(rect.left() + 20.0, y + 9.0),
            egui::Align2::LEFT_CENTER,
            caption,
            FontId::proportional(14.0),
            color::CAPTION,
        );
        y += 24.0;
        let (text, col) = if body.is_empty() {
            ("—", color::SUBTEXT)
        } else {
            (body, color::TEXT)
        };
        let mut job = LayoutJob::single_section(
            text.to_string(),
            TextFormat {
                font_id: FontId::proportional(17.0),
                color: col,
                ..Default::default()
            },
        );
        job.wrap = TextWrapping {
            max_width: wrap,
            max_rows,
            break_anywhere: false,
            overflow_character: Some('…'),
        };
        let g = ui.fonts(|f| f.layout_job(job));
        let h = g.size().y;
        p.galley(pos2(rect.left() + 20.0, y), g, col);
        y += h + 12.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn logical_size() -> egui::Vec2 {
        vec2(LOGICAL_W, LOGICAL_H)
    }

    fn snapshot() -> OverlaySnapshot {
        OverlaySnapshot {
            clipboard_enabled: true,
            auto_input_enabled: false,
            auto_input_send_enter: false,
            vrchat_enabled: true,
            eliza_enabled: false,
            eliza_response_to_vrchat_enabled: false,
            auto_translate_enabled: true,
            is_recording: false,
            last_transcription: "こんにちは".to_string(),
            last_eliza_response: String::new(),
            last_translated_response:
                "Hello, this is a fairly long translated sentence that should wrap onto more lines"
                    .to_string(),
            input: sample_input(),
        }
    }

    fn sample_input() -> InputView {
        use super::super::bridge::ActionView;
        InputView {
            status: InputStatus::Ready,
            actions: vec![
                ActionView {
                    label: "録音を開始",
                    bindings: vec!["左手 十字キー下（2回押し）".to_string()],
                    fired: true,
                },
                ActionView {
                    label: "録音の開始/停止",
                    bindings: vec![],
                    fired: false,
                },
                ActionView {
                    label: "QvPen を呼ぶ",
                    bindings: vec!["右手 B（長押し）".to_string(), "左手 Y".to_string()],
                    fired: false,
                },
            ],
        }
    }

    fn frame(
        ctx: &egui::Context,
        s: &OverlaySnapshot,
        events: Vec<egui::Event>,
    ) -> (Vec<OverlayAction>, Vec<HitTarget>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, logical_size())),
            events,
            ..Default::default()
        };
        let mut out = (Vec::new(), Vec::new());
        let _ = ctx.run(input, |ctx| out = draw(ctx, s));
        out
    }

    fn click(pos: Pos2) -> Vec<egui::Event> {
        let b = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        vec![egui::Event::PointerMoved(pos), b(true), b(false)]
    }

    #[test]
    fn every_control_is_reachable_and_sends_its_action() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let s = snapshot();
        let (_, targets) = frame(&ctx, &s, vec![]);
        // Start/Stop、QvPen、スイッチ 7 つ、割り当てを変える
        assert_eq!(targets.len(), 10);
        for t in &targets {
            let (clicked, _) = frame(&ctx, &s, click(t.rect.center()));
            assert_eq!(clicked, vec![t.action], "{:?} を押したとき", t.action);
        }
    }

    #[test]
    fn layout_fits_in_the_panel_and_targets_do_not_overlap() {
        let ctx = egui::Context::default();
        let mut s = snapshot();
        s.is_recording = true;
        let (_, targets) = frame(&ctx, &s, vec![]);
        let panel = Rect::from_min_size(Pos2::ZERO, logical_size());
        for (i, a) in targets.iter().enumerate() {
            assert!(
                panel.contains_rect(a.rect),
                "{:?} がはみ出している",
                a.action
            );
            assert!(a.rect.height() >= 44.0, "{:?} が押しにくい大きさ", a.action);
            for b in &targets[i + 1..] {
                assert!(
                    !a.rect.intersects(b.rect.shrink(0.5)),
                    "{:?} と {:?} が重なっている",
                    a.action,
                    b.action
                );
            }
        }
    }

    #[test]
    fn switch_order_matches_overlay_fields() {
        let ctx = egui::Context::default();
        let s = snapshot();
        let (_, targets) = frame(&ctx, &s, vec![]);
        let fields: Vec<OverlayAction> = overlay_fields(&s).iter().map(|f| f.action).collect();
        let switches: Vec<OverlayAction> = targets[2..9].iter().map(|t| t.action).collect();
        assert_eq!(switches, fields);
        assert_eq!(targets[9].action, OverlayAction::OpenBindingUi);
    }

    #[test]
    fn binding_card_leaves_room_for_the_text_card() {
        // 割り当てカードが入っても、認識結果のカードに十分な高さが残る
        let ctx = egui::Context::default();
        let s = snapshot();
        let (_, targets) = frame(&ctx, &s, vec![]);
        let open = targets
            .iter()
            .find(|t| t.action == OverlayAction::OpenBindingUi)
            .unwrap();
        // ボタン(カード上端+8) + カードの残り(60-8 + 3行×30 + 14) + 間隔 → 認識結果のカード
        let card_bottom = open.rect.top() - 8.0 + 60.0 + 30.0 * 3.0 + 14.0;
        assert!(
            LOGICAL_H - 20.0 - (card_bottom + 14.0) >= 150.0,
            "認識結果のカードが狭すぎる"
        );
    }
}
