//! デスクトップの設定画面の「SteamVR ボタン割り当て」欄。main.rs と `--dump-overlay` の両方で使う。

use super::bridge::{InputStatus, InputView};

/// 押されたボタン
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingButton {
    /// SteamVR の割り当て画面を PC の画面に開く
    OnDesktop,
    /// VR の中(ダッシュボード)に開く
    InVr,
    /// ブラウザで開く
    Browser,
}

pub fn bindings_section(ui: &mut egui::Ui, view: &InputView) -> Option<BindingButton> {
    match &view.status {
        InputStatus::Starting => {
            ui.label("SteamVR に接続中…");
        }
        InputStatus::Unavailable(why) => {
            ui.label(format!("使えません: {}", why));
        }
        InputStatus::Ready => {
            egui::Grid::new("steamvr_bindings")
                .num_columns(3)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    for a in &view.actions {
                        ui.label(a.label);
                        if a.bindings.is_empty() {
                            ui.weak("未割り当て");
                        } else {
                            ui.label(a.bindings.join(" / "));
                        }
                        if a.fired {
                            ui.colored_label(egui::Color32::from_rgb(0x3d, 0xd6, 0x8c), "● 反応");
                        } else {
                            ui.label("");
                        }
                        ui.end_row();
                    }
                });
        }
    }
    let mut clicked = None;
    ui.horizontal(|ui| {
        if ui.button("割り当てを変える（PC の画面）").clicked() {
            clicked = Some(BindingButton::OnDesktop);
        }
        if ui.button("VR の中で開く").clicked() {
            clicked = Some(BindingButton::InVr);
        }
        if ui.button("ブラウザで開く").clicked() {
            clicked = Some(BindingButton::Browser);
        }
    });
    clicked
}
