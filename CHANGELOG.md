# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2026-10-18

### Added

- コントローラーのボタンで操作できるように（SteamVR のボタン割り当て機能）。VRChat を操作中でも効く。割り当てられる操作は「録音を開始」「録音の開始/停止」「QvPen を呼ぶ」。1回押し・2回押し・長押しなど押し方も選べる
- 既定の割り当て: VRChat のマイクボタンを2回押すと「録音を開始」（Steam Frame は左の十字キー下、Index は左 A、Quest は左 X）
- 録音の開始/終了を VR の中にトーストで表示（目の前の少し下に約2秒。ダッシュボードを閉じていても見える）
- VR の中のオーバーレイと Settings に「キー割り当て変更」ボタン: SteamVR の割り当て画面を開く（オーバーレイからは VR の中、Settings からは PC の画面）

### Removed

- VRChat の OSC でミュート2回を受け取って録音を始める仕組み（ポート 9001 の受信）。上のボタン割り当てで置き換え。VRChat のチャットボックスへの送信（OSC、ポート 9000）はこれまでどおり

## [0.4.0] - 2026-10-18

### Changed

- SteamVR オーバーレイを約2倍の大きさに。表示幅を 2.4m に指定し（以前は指定なしで SteamVR 既定の 1m）、UI は 600×620 の論理サイズを 2 倍の解像度（1200×1240）で描くので文字がくっきり
- オーバーレイの見た目を一新。暗い背景にカード、Start/Stop と QvPen の大きなボタン、チェックボックスの代わりにスイッチ、録音中の表示、指したとき・押したときの色の変化。長い文章は数行で「…」
- 機能・ラベル・押したときの動作は今までどおり

### Added

- 設定 `steamvr_overlay_width_m`（オーバーレイの表示幅、既定 2.4、0.5〜6.0）
- `vrc-companion.exe --dump-overlay <接頭辞>`: SteamVR なしでオーバーレイの見た目を BMP に書き出す（確認用）

## [0.3.0] - 2026-09-03

### Added

- Eliza 経由の自動翻訳 (`auto_translate_enabled`)。VRChat 送信後に翻訳結果を `{原文} / {翻訳}` で再送信。通常会話 (`eliza_enabled`) とは排他
- SteamVR ダッシュボードオーバーレイ (`src/steamvr/`)。OpenVR C API を `libloading` で動的ロードし、隠し WGL コンテキスト + `egui_glow` で描画
- オーバーレイからチェックボックス操作・Start/Stop 録音・QvPen 呼び出しが可能。レーザーポインタ入力 (`SetOverlayInputMethod(Mouse)`) とクリック時ハプティクス
- オーバーレイにタイトル、最終文字起こし、Eliza 応答 / 翻訳結果を表示
- SteamVR 起動時の自動起動登録 (`IVRApplications` + `.vrmanifest`)。キーは `cympfh.vrc_companion`

### Changed

- 設定チェックボックスのラベルを統一
- `make run` は `make install` 経由で `~/apps/vrc-companion/vrc-companion.exe` を実行する

### Fixed

- オーバーレイの Y 座標反転 (実機で上下が逆だった)
- デスクトップウィンドウ非フォーカス時にオーバーレイのクリックが一瞬で戻る問題 (`request_repaint_after`)

(git commit: 3f82667)

## [0.2.0] - 2026-07-01

### Added

- MuteSelf の false→true 切り替え(1秒以内)を検知して録音を自動開始する機能(winh から移植、判定ロジックは `MuteToggleDetector` として切り出しテスト可能にした)

### Changed

- `src/` を `audio/`, `integrations/` サブディレクトリに再編
- CLAUDE.md を追加(アーキテクチャ・開発コマンドの説明)

(git commit: 6f9c34d)

## [0.1.0] - 2026-07-01

### Added

- xAI Speech-to-Text によるストリーミング音声認識(日本語)
- 認識結果のクリップボード自動コピー
- 認識結果のアクティブウィンドウへの自動入力(貼り付け/直接入力、Enter 送信オプション)
- 認識結果の VRChat チャットボックスへの OSC 送信
- 認識結果の Eliza(対話エージェント)への送信、応答の GUI 表示、応答の VRChat への送信
- QvPen 呼び出しボタン

(git commit: e461a3b)
