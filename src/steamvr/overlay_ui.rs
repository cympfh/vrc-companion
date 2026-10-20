//! SteamVR オーバーレイの中身(egui)。描画先(WGL / テスト)に依存しないので全OSでテストできる。
//!
//! 見た目: 暗い背景にカードを並べる。上から、見出し(右上に小さな「キー割り当て」ボタンと録音中の表示) →
//! Start/Stop と QvPen の大きなボタン → 送り先・Eliza の2枚のカード(各行は右端にスイッチ) →
//! 認識結果のカード。右端には丸い A/B/X/Y ボタンが縦に4つ並ぶ(アバターパラメーターのオン/オフ)。
//! 座標はすべて論理ピクセル(`LOGICAL_W` × `LOGICAL_H`)。実際のテクスチャは
//! `PIXELS_PER_POINT` 倍の解像度で描くので、大きく表示しても文字がぼやけない。

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Color32, FontId, Pos2, Rect, Rounding, Sense, Stroke, pos2, vec2};

use super::bridge::{
    AvatarButtonView, OverlayAction, OverlayField, OverlaySnapshot, overlay_fields,
};
use crate::avatar_buttons::LABELS;

/// UI の論理サイズ(マウス座標もこの単位)
pub const LOGICAL_W: f32 = 600.0;
pub const LOGICAL_H: f32 = 620.0;
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
}

const PAD: f32 = 20.0;
const GAP: f32 = 14.0;
const RADIUS: f32 = 14.0;
/// 右端の丸いボタンの直径
const AVATAR_D: f32 = 62.0;

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
    // レーザーポインタは押している間も手ぶれで動く。egui の既定(6pt 動いたら、0.8 秒押したら
    // クリックにしない)だと、少しでも動くとクリックが消える。このパネルにはドラッグもスクロールも
    // 長押しも無いので、押した部品の上で離せばクリックにする(外で離したら何もしない: `released_on`)。
    ctx.options_mut(|o| {
        o.input_options.max_click_dist = f32::INFINITY;
        o.input_options.max_click_duration = f64::INFINITY;
    });
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

            // 見出し。右上の小さなボタンで SteamVR のキー割り当て画面を VR の中に開く
            let bind_rect = header(
                ui,
                snapshot,
                Rect::from_min_size(pos2(left, y), vec2(width, 40.0)),
            );
            if small_button(ui, "open_binding_ui", bind_rect, "キー割り当て") {
                clicked.push(OverlayAction::OpenBindingUi);
            }
            targets.push(HitTarget {
                action: OverlayAction::OpenBindingUi,
                rect: bind_rect,
            });
            y += 40.0 + GAP;

            // 右端の A/B/X/Y。残りはその左に並べる
            let mut ay = y;
            for (i, view) in snapshot.avatar_buttons.iter().enumerate() {
                let rect = Rect::from_min_size(
                    pos2(left + width - AVATAR_D, ay),
                    vec2(AVATAR_D, AVATAR_D),
                );
                let action = OverlayAction::ToggleAvatarButton(i);
                if avatar_button(ui, i, rect, *view) {
                    clicked.push(action);
                }
                if view.configured {
                    targets.push(HitTarget { action, rect });
                }
                ay += AVATAR_D + GAP;
            }
            let strip_bottom = ay - GAP;
            let full_width = width;
            let width = width - AVATAR_D - GAP;

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

            // 認識結果。丸いボタンより下なら横いっぱいに使う
            let text_w = if y >= strip_bottom + GAP {
                full_width
            } else {
                width
            };
            let text_rect =
                Rect::from_min_max(pos2(left, y), pos2(left + text_w, full.bottom() - PAD));
            text_card(ui, snapshot, text_rect);
        });
    (clicked, targets)
}

/// 見出しを描き、録音中の表示のすぐ左に置く小さなボタンの場所を返す。
fn header(ui: &egui::Ui, snapshot: &OverlaySnapshot, rect: Rect) -> Rect {
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
    let label_w = ui.fonts(|f| {
        f.layout_no_wrap(
            "キー割り当て".to_string(),
            FontId::proportional(14.0),
            color::TEXT,
        )
        .size()
        .x
    });
    let btn_w = label_w + 28.0;
    Rect::from_center_size(
        pos2(pill.left() - 8.0 - btn_w / 2.0, rect.center().y),
        vec2(btn_w, 32.0),
    )
}

/// 見出しに置く控えめなボタン(枠線だけ)。押されたら true。
fn small_button(ui: &mut egui::Ui, id: &str, rect: Rect, label: &str) -> bool {
    let resp = ui.interact(rect, ui.id().with(id), Sense::click());
    let p = ui.painter();
    let fill = if resp.is_pointer_button_down_on() {
        Color32::from_white_alpha(30)
    } else if resp.hovered() {
        Color32::from_white_alpha(16)
    } else {
        Color32::TRANSPARENT
    };
    p.rect(
        rect,
        Rounding::same(16.0),
        fill,
        Stroke::new(1.0, color::CARD_BORDER),
    );
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(14.0),
        color::SUBTEXT,
    );
    released_on(&resp)
}

/// 丸い A/B/X/Y ボタン。オンは青く塗って光の輪、オフは暗い地に枠線、未設定は薄く押せない
fn avatar_button(ui: &mut egui::Ui, i: usize, rect: Rect, view: AvatarButtonView) -> bool {
    let c = rect.center();
    let r = rect.width() / 2.0;
    let p = ui.painter().clone();
    let label = LABELS[i];
    if !view.configured {
        p.circle(c, r - 1.0, color::BG, Stroke::new(1.0, color::CARD_BORDER));
        p.text(
            c,
            egui::Align2::CENTER_CENTER,
            label,
            FontId::proportional(22.0),
            color::TRACK_OFF,
        );
        return false;
    }
    let resp = ui.interact(rect, ui.id().with(("avatar", i)), Sense::click());
    let t = ui
        .ctx()
        .animate_bool_with_time(ui.id().with(("avatar_on", i)), view.on, 0.12);
    if t > 0.0 {
        p.circle_filled(c, r, color::ACCENT.gamma_multiply(0.35 * t));
    }
    let fill = lerp_color(color::CARD, color::ACCENT, t);
    let border = lerp_color(color::SUBTEXT, color::ACCENT, t);
    p.circle(c, r - 5.0, fill, Stroke::new(2.0, border));
    if resp.is_pointer_button_down_on() {
        p.circle_filled(c, r - 5.0, Color32::from_black_alpha(60));
    } else if resp.hovered() {
        p.circle_filled(c, r - 5.0, Color32::from_white_alpha(22));
    }
    p.text(
        c,
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(24.0),
        if view.on { Color32::WHITE } else { color::TEXT },
    );
    released_on(&resp)
}

/// 押した部品の上で離したときだけ true(手ぶれは許すが、外へ動かして離したら押さない)
fn released_on(resp: &egui::Response) -> bool {
    resp.clicked()
        && resp
            .ctx
            .input(|i| i.pointer.interact_pos())
            .is_some_and(|p| resp.rect.contains(p))
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
    released_on(&resp)
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
        if released_on(&resp) {
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
            avatar_buttons: [
                AvatarButtonView {
                    configured: true,
                    on: true,
                },
                AvatarButtonView {
                    configured: true,
                    on: false,
                },
                AvatarButtonView {
                    configured: true,
                    on: false,
                },
                AvatarButtonView::default(),
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
        // キー割り当て、A/B/X(Y は未設定)、Start/Stop、QvPen、スイッチ 7 つ
        assert_eq!(targets.len(), 13);
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
            // 見出しの小さなボタンだけは 32 まで許す
            let min_h = if a.action == OverlayAction::OpenBindingUi {
                32.0
            } else {
                44.0
            };
            assert!(
                a.rect.height() >= min_h,
                "{:?} が押しにくい大きさ",
                a.action
            );
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
        assert_eq!(targets[0].action, OverlayAction::OpenBindingUi);
        let switches: Vec<OverlayAction> = targets[6..13].iter().map(|t| t.action).collect();
        assert_eq!(switches, fields);
    }

    #[test]
    fn binding_button_sits_in_the_header_left_of_the_status() {
        // 録音中/待機中のどちらでも、見出しの行に収まり、表示とも見出しの文字とも重ならない
        let ctx = egui::Context::default();
        for rec in [false, true] {
            let mut s = snapshot();
            s.is_recording = rec;
            let (_, targets) = frame(&ctx, &s, vec![]);
            let open = targets[0].rect;
            let rec_btn = targets[4].rect;
            assert!(open.bottom() < rec_btn.top(), "見出しの行に無い");
            assert!(open.right() < LOGICAL_W - 20.0 - 60.0, "状態表示と重なる");
            assert!(open.left() > 20.0 + 200.0, "見出しの文字と重なる");
            assert!(open.width() < 160.0, "小さなボタンになっていない");
        }
    }

    #[test]
    fn text_card_keeps_enough_room() {
        // 一番下のスイッチの下に、認識結果のカードの高さが十分残る
        let ctx = egui::Context::default();
        let s = snapshot();
        let (_, targets) = frame(&ctx, &s, vec![]);
        let bottom = targets.iter().map(|t| t.rect.bottom()).fold(0.0, f32::max);
        assert!(
            LOGICAL_H - 20.0 - (bottom + 8.0 + 14.0) >= 150.0,
            "認識結果のカードが狭すぎる"
        );
    }

    #[test]
    fn avatar_buttons_are_a_column_at_the_right_edge() {
        let ctx = egui::Context::default();
        let mut s = snapshot();
        s.avatar_buttons = [AvatarButtonView {
            configured: true,
            on: false,
        }; 4];
        let (_, targets) = frame(&ctx, &s, vec![]);
        let av: Vec<Rect> = (0..4)
            .map(|i| {
                targets
                    .iter()
                    .find(|t| t.action == OverlayAction::ToggleAvatarButton(i))
                    .unwrap()
                    .rect
            })
            .collect();
        let header_bottom = targets[0].rect.bottom();
        for (i, r) in av.iter().enumerate() {
            assert_eq!(r.right(), LOGICAL_W - 20.0, "右端に揃っていない");
            assert!(r.top() > header_bottom, "見出しと重なる");
            if i > 0 {
                assert!(r.top() > av[i - 1].bottom(), "上から A B X Y の順でない");
            }
        }
    }

    #[test]
    fn unconfigured_avatar_button_is_not_clickable() {
        let ctx = egui::Context::default();
        let s = snapshot();
        let (_, targets) = frame(&ctx, &s, vec![]);
        assert!(
            !targets
                .iter()
                .any(|t| t.action == OverlayAction::ToggleAvatarButton(3))
        );
        // Y のあった場所を押しても何も起きない
        let x = targets
            .iter()
            .find(|t| t.action == OverlayAction::ToggleAvatarButton(2))
            .unwrap()
            .rect;
        let y_center = x.center() + vec2(0.0, AVATAR_D + 14.0);
        let (clicked, _) = frame(&ctx, &s, click(y_center));
        assert!(clicked.is_empty());
    }

    /// 押す → 手ぶれで何度か動く → 離す、を1フレームずつ送る(時刻も進める)
    fn jittery_click(
        ctx: &egui::Context,
        s: &OverlaySnapshot,
        press: Pos2,
        moves: &[egui::Vec2],
        release: Pos2,
        seconds: f64,
    ) -> Vec<OverlayAction> {
        let b = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let mut frames = vec![vec![egui::Event::PointerMoved(press), b(press, true)]];
        for m in moves {
            frames.push(vec![egui::Event::PointerMoved(press + *m)]);
        }
        frames.push(vec![egui::Event::PointerMoved(release), b(release, false)]);
        let n = frames.len() as f64;
        let mut all = Vec::new();
        for (i, events) in frames.into_iter().enumerate() {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, logical_size())),
                time: Some(10.0 + seconds * i as f64 / (n - 1.0)),
                events,
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| all.extend(draw(ctx, s).0));
        }
        all
    }

    #[test]
    fn click_survives_pointer_jitter_on_every_control() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let s = snapshot();
        let (_, targets) = frame(&ctx, &s, vec![]);
        let jitter = [vec2(3.0, -2.0), vec2(-7.0, 5.0), vec2(9.0, 8.0), vec2(-4.0, -9.0)];
        for t in &targets {
            let c = t.rect.center();
            let release = c + vec2(t.rect.width() * 0.3, t.rect.height() * 0.3);
            let clicked = jittery_click(&ctx, &s, c, &jitter, release, 0.3);
            assert_eq!(clicked, vec![t.action], "{:?} を手ぶれしながら押したとき", t.action);
        }
    }

    #[test]
    fn slow_trigger_pull_still_clicks() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let s = snapshot();
        let (_, targets) = frame(&ctx, &s, vec![]);
        let t = targets[4];
        let c = t.rect.center();
        assert_eq!(
            jittery_click(&ctx, &s, c, &[vec2(2.0, 1.0)], c, 1.5),
            vec![t.action]
        );
    }

    #[test]
    fn releasing_outside_the_pressed_control_does_nothing() {
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let s = snapshot();
        let (_, targets) = frame(&ctx, &s, vec![]);
        let t = targets[4];
        let c = t.rect.center();
        // 押したまま別の場所(見出しの文字の上)へ動かして離す
        let away = pos2(60.0, 40.0);
        assert!(targets.iter().all(|t| !t.rect.contains(away)));
        let clicked = jittery_click(&ctx, &s, c, &[vec2(20.0, -40.0)], away, 0.3);
        assert!(clicked.is_empty(), "{clicked:?}");
    }
}
