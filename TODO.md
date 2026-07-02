## [x] とりあえず動くものを実装する [2026-07-01 16:25 完了]

事前知識として [winh](~/git/winh/) を読むこと

- winh から以下の機能だけを抽出して、最小限のコードで動くものを作る
    1. 音声認識 (speech-to-text)
    2. テキストをクリップボードに入れるチェックボックス
    3. テキストを active window に送信するチェックボックス
    4. (3がtrueのとき) Enter キーを最後に送るかどうかのチェックボックス
    5. (3とは排反に) テキストを VRChat に送信するチェックボックス
    6. テキストを Eliza に送信するチェックボックス (3や5とは独立)
    6. Eliza からの返答を VRChat に送信する機能
    7. call QvPen ボタン

開発は TDD. テストを最初にかく.

- 実装: src/{config,audio,speech_to_text,vrchat,eliza,auto_input}.rs + main.rs
    - winh からグローバルホットキー・VRChat ミュートトリガー・入力デバイス選択・アイコンを省いて最小化
    - `cargo test` で14件のユニットテスト通過、`cargo build` 成功
    - GUI起動確認: この WSL 環境は GPU パススルーが無く `LIBGL_ALWAYS_SOFTWARE=1` が必要だったが、それを付けるとクラッシュせず起動した(スクリーンショット取得ツールが環境に無く目視確認は未実施)

## [x] README.md, Makefile [2026-07-01 16:31 完了]

README.md -- 利用者向け（日本語）
Makefile -- 開発者向け (make build, make run, make test, make clean)

- README.md: 概要・機能一覧・必要なもの(xAI API Key, VRChat OSC, Eliza)・設定ファイル場所・使い方・ビルド方法を記載
- Makefile: build/run/test/clean に加え fmt/lint も用意。`make test` で14件のテストが通ることを確認

## [x] feat [2026-07-01 16:53 完了]

### 機能追加

- Eliza の返答をGUIに表示する
    - speech-to-text の結果の下に同様に表示する
- Eliza からの返答を VRChat に送信するかどうか選べるチェックボックス

### 機能変更

Eliza に送信チェックを入れてても、右手の hand gesture を追加しないと実際には送信してないのが現状

チェックが入ってたら全部送ることにする。

- 実装状況の確認: Eliza の返答をGUIに表示する機能はすでに実装済みだった（main.rs の eliza_response 表示部分）
- 新規実装: `eliza_response_to_vrchat_enabled` を Config に追加し、「Send Eliza's response to VRChat」チェックボックスを追加（Send to Eliza が有効な時のみ操作可能）
    - 従来は Eliza 応答の VRChat 送信が `vrchat_enabled`（ユーザー発話の VRChat 送信フラグ）に依存していたため、独立させた
    - このチェックボックスが true なら条件なしで送信するようにした（`cargo test` で16件のテスト通過、`cargo build` 成功）

## [x] リファクタリング [2026-07-01 17:01 完了]

- dead code は消す
- 有用でないコメントは消す
- fmt に掛ける
- テストが通ることを確認する
- 冗長や一貫性のない変数名/関数名を整理する
- 複雑過ぎるロジックは関数に切り出す

- `cargo clippy --fix` で collapsible-if を let-chain 化（main.rs, audio.rs）
- src/ をジャンル別サブディレクトリに再編（ユーザー追加要望）
    - `src/audio/{mod.rs, speech_to_text.rs}` (録音・音声認識)
    - `src/integrations/{vrchat.rs, eliza.rs, auto_input.rs}` (外部連携)
    - `config.rs`, `main.rs` はルートに残す
- `auto_input.rs`: Enigo インスタンス生成 (`new_enigo`) と Enter 送信 (`press_enter`) の重複コードを関数抽出
- `cargo fmt` / `cargo build` / `cargo test` (15件通過) 確認済み
- 残存 clippy warning (`too_many_arguments`, `field_reassign_with_default` in tests, `enum_variant_names`) はスタイル上の些末な指摘のため対応せず

## [x] winh 同様に MuteSelf トリガーで Start する機能 [2026-07-01 17:09 完了]

9001 で OSC 受信して、`/avatar/parameters/MuteSelf` のくいくいってやったら Start ボタンを押すのと同じことをする
winh にある機能なので基本そのまま持ってきて

- `src/integrations/vrchat.rs`: winh の `start_mute_listener` を移植
    - False→True (ミュート解除→ミュート) が1秒以内に起きたら「くいくい」判定
    - 判定ロジックを `MuteToggleDetector` として切り出し、`with_timeout` でタイムアウトを注入できるようにしてユニットテスト可能にした（winh は判定ロジックがリスナー関数にインライン化されテスト不可だったため改善）
    - winh の GestureRight/eliza_gesture 連動 (mute トリガー時に手のジェスチャーで eliza モードを切り替える機能) は vrc-companion に該当する概念(`config.eliza_gesture`)が無いため持ってこず、単純な「録音開始トリガー」のみ移植
- `src/main.rs`: `App` に `mute_trigger_receiver: Receiver<()>` を追加、`App::new` でリスナーを起動、`update()` で受信したら Start ボタンと同じ `on_start_recording()` を呼ぶ（録音中/認識中は無視）
- `cargo test` で20件（新規5件）のテスト通過、`cargo build` 成功、`cargo fmt` 適用済み

## [x] feat: 翻訳機能 [2026-07-01 17:20 完了]

- 5 の VRChat に送信するチェックボックスの下に
    - 自動翻訳をするのチェックボックスを追加する
- チェックすると
    - 翻訳先言語を選べるドロップダウン (EN/CN/自由記述)が登場
    - speech-to-text する → 通常通り VRChat に送信する → Eliza に翻訳依頼する → 翻訳結果を得る → `{元テキスト} / {翻訳結果}` を VRChat に送信する

```
curl -s -X POST http://localhost:9096/eliza/api/translate -H "Content-Type: application/json" -d '{"source_lang":"日本語","target_lang":"英語","text":"こんにちわ"}'
{"translated_text":"Hello"}
```

localhost:9096 は Eliza API Server に書き換えてね

GUI にも「Elizaからの返答」の代わりに「Elizaからの翻訳結果」を表示する

- `Config` に `auto_translate_enabled` / `translate_lang_preset` (EN/CN/CUSTOM) / `translate_lang_custom` を追加、`translate_target_lang()` で EN→英語, CN→中国語, CUSTOM→自由記述テキストに解決
- `ElizaClient::translate(source_lang, target_lang, text)` を追加 (`/eliza/api/translate` へPOST、`translated_text` を返す)。既存の `send_chat`（`/eliza/api/chat`, item 6の会話機能）とは独立したエンドポイント・独立したチェックボックス
- `App::on_transcription_success`: `auto_translate_enabled` なら別スレッドで `translate("日本語", target_lang, text)` を呼び、結果を `translate_response_receiver` へ送信。既存の `vrchat_enabled` ゲートの原文送信はそのまま変更せず
- `App::update()`: 翻訳結果を受信したら `{元テキスト} / {翻訳結果}` を VRChat に送信（`eliza_response_to_vrchat_enabled` と同様に、既存の `vrchat_enabled` トグルとは独立して送信する設計 — TODO記載の Eliza応答→VRChat送信の decouple パターンを踏襲）
- GUI: 「Send to VRChat」の下に「自動翻訳する」チェックボックス + 有効時のみ表示される言語ドロップダウン(EN/CN/自由記述)、自由記述選択時はテキスト入力欄も表示。表示エリアには「Eliza:」ブロックとは別に「Elizaからの翻訳結果:」ブロックを追加（`auto_translate_enabled` かつ結果が空でない時のみ表示）
- テスト: `config.rs` に2件 (`translate_target_lang` の preset解決、serdeラウンドトリップ)、`integrations/eliza.rs` に3件 (endpoint生成、リクエストシリアライズ、レスポンスデシリアライズ) 追加、`cargo test` で25件（新規5件）通過、`cargo build` / `cargo fmt` 確認済み
- `cargo clippy --all-targets -- -D warnings` は既存の3件の警告（`too_many_arguments` in audio/mod.rs, `field_reassign_with_default` in config.rs tests, `enum_variant_names` in speech_to_text.rs）が変更前から存在済みであることを確認（`git stash` で検証）。今回の変更で新規の clippy エラーは無い
- 追加要望: 自動翻訳(`auto_translate_enabled`)と Send to Eliza(`eliza_enabled`) を排他にした。`enable_eliza_exclusive()` / `enable_auto_translate_exclusive()` を追加し、既存の auto_input/vrchat 排他パターンを踏襲。`cargo test` 26件通過

## [x] SteamVR でも動くようにする！ [2026-07-01 19:07 完了 (Stage1のみ、Stage2-4は下記サブタスクへ)]

今までどおり GUI は残すが, SteamVR の overlay にも表示できるようにする
設定画面は不要（それはPCのGUIからやる）
各種チェックボックスとか QvPen ボタンだけが並んでれば最高

- OpenVR公式C++ SDK(既存crateの`openvr`/`ovr_overlay`含む)はMSVC vtable ABIで、このプロジェクトのクロスコンパイル先`x86_64-pc-windows-gnu`と非互換のため使用不可。代わりにOpenVRの**プレーンC API**(`openvr_api.dll`)を`libloading`で動的ロードし、`VR_GetGenericInterface("FnTable:IVROverlay_028", ...)`でABI安全な関数ポインタテーブルを取得する自作FFIで実装（ユーザーの明示決定:「自作を試みましょう。動作確認は私がやりますから」）
- `src/steamvr/` モジュールを新設: `bridge.rs`(全OS対象、`OverlaySnapshot`/`OverlayAction`/`OverlayHandle`型 + `start()`。non-windowsは常にNone)、`ffi.rs`(`#[cfg(windows)]`、`openvr_capi.h`から逐語転記した82フィールドの`VR_IVROverlay_FnTable`、`VR_InitInternal`/`VR_GetGenericInterface`/`VR_ShutdownInternal`の生FFI)、`session.rs`(`#[cfg(windows)]`、init→`CreateDashboardOverlay`→単色RGBAプレースホルダを`SetOverlayRaw`で表示するだけ)
- `App`に`steamvr_overlay: Option<OverlayHandle>`を追加。`App::new`で`steamvr::start()`(失敗時はeprintln!してNone、既存の`vrchat::start_mute_listener`と同様に非致命的)。`App::update()`で`action_rx`をdrainして`apply_steamvr_action()`(既存の`Config`排他制御メソッドを再利用)、デスクトップGUI側のチェックボックス変更検知ブロックからも`push_steamvr_snapshot()`でVR側に反映
- Cargo.toml: `[target.'cfg(windows)'.dependencies]`に`libloading = "0.8"`追加
- 今回のスコープはStage 1（単色プレースホルダをダッシュボードオーバーレイに表示するところまで）。実際のegui描画・チェックボックス入力処理は未実装（下記サブタスク参照）
- `cargo test`(28件、host)/`cargo build`(host)/`cargo build --target x86_64-pc-windows-gnu`（Windows-gated FFIコードの実コンパイル確認、警告ゼロ）/`cargo fmt --check` 全て確認済み
- **ユーザー確認: 済 [2026-07-01 21:22]** — 実機(Windows+SteamVR)でダッシュボードオーバーレイにプレースホルダの水色/青色矩形が表示されることを確認
- events.rs(OpenVRイベント→OverlayAction変換)は当初計画にあったが、Stage1では実際の呼び出し元(`PollNextOverlayEvent`配線)が無く`dead_code`になるため、Stage3に先送りした（意図的な計画からの逸脱）

### サブタスク（Stage1のユーザー確認後、個別に `/todo` で対応）

- [x] Stage 2: `steamvr::render` [2026-07-01 22:14 完了、ユーザー確認: 未] — プレースホルダの単色画像を実際のegui描画に置き換えた
  - `glutin`は使わず自作WGL(`windows-sys`)で隠しウィンドウ+コンテキストを直接作成。理由: `eframe`がプロセス内で唯一の`winit::EventLoop`を既に保持しており、winitは2つ目の`EventLoop`生成をプロセス全体のフラグでエラーにするため（macOS限定ではなく全プラットフォーム共通の制約）。Stage1の「生FFIを手で書く」路線を継続
  - 新規`src/steamvr/render.rs`(`#[cfg(windows)]`): `GlOverlayRenderer` — 隠しウィンドウ→`ChoosePixelFormat`/`SetPixelFormat`→`wglCreateContext`/`wglMakeCurrent`→`glow::Context`(GL関数ロードは`wglGetProcAddress`優先、nullなら`opengl32.dll`の`GetProcAddress`にフォールバック)→FBO+RGBAテクスチャ→`egui_glow::Painter`。`render()`は`bridge::overlay_fields()`を全て`ui.add_enabled(false, ...)`で読み取り専用チェックボックス表示+無効化した「📝 call QvPen」ボタンを描画し`gl.flush()`後にテクスチャを返す
  - `ffi.rs`: `Texture_t`構造体(`openvr_capi.h`から逐語確認、16バイト)追加、`set_overlay_texture`フィールドをプレースホルダから実シグネチャに変更
  - `bridge.rs`: `OverlayField`/`overlay_fields()`追加（デスクトップGUIのチェックボックスと同じラベル・順序・インデントを反映する純粋関数、全OSでテスト可能）
  - `session.rs`: `show_placeholder`/`SetOverlayRaw`を削除、`GlOverlayRenderer`初期化+ループのタイムアウトを500ms→33msに変更、毎tick`render()`→`SetOverlayTexture`
  - `cargo test`(30件、host)/`cargo build`(host)/`cargo build --target x86_64-pc-windows-gnu`(警告ゼロ)/`cargo fmt --check` 全て確認済み。`cargo clippy -- -D warnings`は元から(Stage2以前から)repo全体で失敗しており、Stage2で追加した`render.rs`/`session.rs`/`ffi.rs`由来の新規エラーは無いことを`git stash`で確認済み（clippy自体の全面修正はスコープ外）
  - このWSL環境はGPUパススルー無しのため、WGL/GL/FBO/テクスチャ受け渡しの実動作はここでは検証不能。クロスコンパイル通過が本セッションでの検証の限界。「映らない/おかしい」場合は追加のフィードバックループが必要になる可能性がある旨は実装前から想定済み
  - **実機フィードバック [2026-07-01]**: 「表示されたが文字とチェックボックス等の描画だけ。コントローラで操作はできない。日本語は文字化け(豆腐)」
    - 日本語文字化け→修正済み: `GlOverlayRenderer`が独自に`egui::Context::default()`を作っており、`main.rs`がデスクトップウィンドウ用の`egui::Context`に読み込んでいる日本語フォント(`fonts/NotoSansJP-Regular.ttf`)がこの別コンテキストには一切適用されていなかった。`render.rs`に`setup_fonts()`を追加し`GlOverlayRenderer::new`内で同フォントを読み込むよう修正。`cargo build --target x86_64-pc-windows-gnu`警告ゼロで再確認済み（実機再確認は未）
    - コントローラ操作不可→仕様通り: Stage2は全ウィジェットを`add_enabled(false, ...)`で読み取り専用にしており、`PollNextOverlayEvent`による入力配線も未実装（Stage3で対応予定）。バグではない
- [x] Stage 3: `steamvr::events` [2026-07-01 22:56 完了、ユーザー確認: 未] — `PollNextOverlayEvent`で入力イベントを取得しegui入力に変換、`OverlayAction`の実配線（クリック→送信）を実装
  - `ffi.rs`: `openvr_capi.h`から逐語確認した`VREvent_t`/`VREvent_Mouse_t`/`VREvent_Data_t`(union、実際に使う`mouse`とサイズ/アライメント合わせ用の`reserved: [u64; 6]`のみ転記)を追加、`poll_next_overlay_event`フィールドをプレースホルダから実シグネチャに変更。サイズトリップワイヤーテスト追加(64バイト、`Texture_t`と同パターン)
  - `bridge.rs`: `OverlayField`に`action: OverlayAction`を追加し`overlay_fields()`の7エントリ全てに対応するアクションを設定。`map_mouse_button`(raw button code→`egui::PointerButton`、host testable)を追加。`OverlayAction`の`#[allow(dead_code)]`は削除せず維持——実際の construct 元(`render.rs`)・呼び出し元(`session.rs`)が`#[cfg(windows)]`のため非Windows host buildでは(テスト以外)到達不能で`dead_code`が必ず出る構造だと判明したため、Stage3計画にあった「実構築先ができたら削除する」という想定は誤りだった。`map_mouse_button`にも同理由で`#[allow(dead_code)]`を追加
  - `render.rs`: `render()`が`events: Vec<egui::Event>`を受け取り`(NativeTexture, Vec<OverlayAction>)`を返すように変更。`add_enabled(false, ...)`の読み取り専用ウィジェットを実際の`ui.checkbox`/`ui.button`に変更し`.clicked()`で`OverlayAction`を収集
  - `session.rs`: `poll_next_overlay_event`を空になるまでポーリングし`egui::Event`(`PointerMoved`/`PointerButton`)に変換する`poll_overlay_events()`を追加。メインループで毎tick呼び出し、クリックされた`OverlayAction`を`action_tx`経由で送信。`main.rs`側のdrain/`apply_steamvr_action`/snapshot再送は既存配線のまま変更不要だった
  - Y座標反転(`height - mouse.y`、OpenVRのOpenGL式座標系→egui座標系)は未検証の仮定。実機で上下が逆だった場合は`session.rs`の該当1行を外すだけで直るようコメントを残した
  - `cargo test`(32件、host)/`cargo build`(host)/`cargo build --target x86_64-pc-windows-gnu`(警告ゼロ)/`cargo fmt --check` 全て確認済み。`cargo clippy --all-targets -- -D warnings`は`git stash`比較でStage2ベースラインと完全に同一のエラー集合であることを確認済み(新規リグレッション無し)——ただし`make lint`自体はStage2以前から既に失敗している状態(`OverlayField`/`overlay_fields`が host build で dead_code、`all_variants_same_postfix`、`too_many_arguments`、`field_reassign_with_default`×4、`type_complexity`)で、これはStage2でも承知の上でスコープ外とされていた既存debtであり今回も対応していない
  - このWSL環境はSteamVR実行不可のため、レーザーポインタ/コントローラで実際にクリックできるか・Y座標反転の向きが正しいかは実機でのユーザー確認が必要
  - **実機フィードバック [2026-07-02]**: 「変わらずUIが表示だけされてコントローラで操作はできない」
    - 根本原因判明・修正済み: `SetOverlayInputMethod`を一度も呼んでいなかった。OpenVRのオーバーレイは`InputMethod`が`None`(デフォルト値)のままだと、`PollNextOverlayEvent`によるイベント取得ロジックが正しく実装済みでも、SteamVRコンポジタ自体がそのオーバーレイに対して`MouseMove`/`MouseButtonDown`/`MouseButtonUp`を一切生成しない——Stage3の入力配線コードは構造的に正しかったが、その前段の有効化呼び出しが欠けていたため何をやっても反応しないのは仕様上当然だった。公式Qtサンプル(`samples/helloworldoverlay/openvroverlaycontroller.cpp`)の`Init()`と同じ呼び出し順序(`CreateDashboardOverlay`→`ShowOverlay`成功後に`SetOverlayInputMethod(Mouse)`)で修正
    - 併せて`SetOverlayMouseScale`も追加。理由: `VREvent_Mouse_t.x/y`が報告される座標空間の範囲を明示的に指定するAPIで、公式サンプルもレンダーサイズ確定時に必ず呼んでいる。指定しないと座標空間の対応が保証されず、既存のY反転(`height - mouse.y`)が想定する400×300ピクセル空間と一致しない可能性がある
    - `ffi.rs`: `HmdVector2_t`(`{v: [f32;2]}`、8バイト、`openvr_capi.h`逐語確認)追加、サイズトリップワイヤーテスト追加。`EVROverlayInputMethod`型・`INPUT_METHOD_MOUSE=1`定数追加。`set_overlay_input_method`/`set_overlay_mouse_scale`フィールドをプレースホルダから実シグネチャに変更（`get_overlay_*`側は呼び出し不要のためプレースホルダのまま維持）
    - `session.rs`: `set_overlay_input_method`/`set_overlay_mouse_scale`ヘルパー関数を`show_overlay`と同じエラーハンドリング形式で追加、`run()`内`show_overlay`成功後・`GlOverlayRenderer`初期化前に呼び出し
    - `cargo test`(32件)/`cargo build`/`cargo build --target x86_64-pc-windows-gnu`(警告ゼロ)/`cargo fmt --check`全て確認済み。`cargo clippy --all-targets -- -D warnings`はStage3時点のエラー集合と`git stash`比較で完全一致(新規リグレッション無し)
    - このWSL環境はSteamVR実行不可のため、この修正で実際に操作できるようになるか・Y座標反転の向きが正しいかは改めて実機でのユーザー確認が必要
  - **実機フィードバック [2026-07-02 続報]**: 「1. 縦方向の座標が逆転してる (h -> 1-h). 2. 押しても実際には反応してない」
    - Y座標反転→修正済み: `session.rs::poll_overlay_events`の`height as f32 - mouse.y`を`mouse.y`に変更(反転を撤去)。ユーザーの「h -> 1-h」という報告は既存の反転がちょうど逆方向であることを直接示しており、Stage3実装時のコメントで明記していた「反転が逆だったらこの1行を外すだけで直る」の通りの対応。併せて`height`パラメータが不要になったため関数シグネチャからも削除(呼び出し元`run()`も2引数に修正)
    - クリック無反応→`mouse.button`のビット値デコードへの依存を撤去して修正: `render.rs`のUIは`Response::clicked()`(Primaryボタン限定)しか見ておらず、ダッシュボードのレーザーポインタ操作は実質「トリガー押下=クリック」の単一ボタン操作。従来は`bridge::map_mouse_button(mouse.button)`で1/2/4→Primary/Secondary/Middleを判定し、判定できた場合のみ`PointerButton`イベントを送っていたが、(a)`MouseButtonDown`時に`button`がPrimary(1)以外の値になっていた場合、あるいは(b)`MouseButtonUp`時に`button`フィールドが押下時と一致しない/正しく載っていない場合、どちらでもクリックとして成立しなくなる構造的な脆さがあった。ボタン種別の判定自体をやめ、`MouseButtonDown`/`MouseButtonUp`を常に`egui::PointerButton::Primary`として送るように変更
    - `bridge.rs`: 呼び出し元が無くなった`map_mouse_button`関数と対応するテスト2件を削除(未使用コードは残さない方針)。付随して不要になった`use super::bridge::{self, ...}`の`self`インポートも削除(clippy未使用importエラー回避)
    - `cargo test`(30件、host。`map_mouse_button`関連2件減)/`cargo build`(host)/`cargo build --target x86_64-pc-windows-gnu`(警告ゼロ)/`cargo fmt --check`全て確認済み。`cargo clippy --all-targets -- -D warnings`は`git stash`比較でStage3ベースラインと完全に同一のエラー集合であることを確認済み(新規リグレッション無し)
    - このWSL環境はSteamVR実行不可のため、Y座標が正しい向きになったか・クリックが実際に反応するようになったかは改めて実機でのユーザー確認が必要
  - **実機フィードバック [2026-07-02 続報2]**: 「Y座標は治った。クリックは一瞬だけチェックボックスにチェックが入るが一瞬だけで、すぐに消える」
    - Y座標修正は確認完了。クリックの「一瞬だけ反映されすぐ消える」現象の根本原因判明・修正済み: `main.rs`の`eframe::App::update()`は`is_recording`/`is_transcribing`のときだけ`ctx.request_repaint()`する反応的(reactive)設計で、それ以外のアイドル時は次のOS入力/描画イベントが来るまで`update()`自体が呼ばれない。`steamvr_overlay.action_rx`のdrain(Config反映→`config.save()`→`push_steamvr_snapshot()`)はこの`update()`内でしか実行されないため、ユーザーがVR内にいてデスクトップウィンドウを見ていない間(オーバーレイのチェックボックスを操作したい状況そのもの)は、クリックしてもConfig反映・スナップショット再送が長時間止まる。その間`session.rs`側は33msごとに古いスナップショット(`latest`)で再描画を続けるため、クリック直後の1フレームだけ`render.rs`内のローカル`checked`変数がeguiの`.clicked()`処理で一時的にtrueへ反転して見えるが、次のtickからは再び古い(false)スナップショットで描画され戻ってしまい、実際のConfig反映がいつ届くか(あるいは届かないか)に関わらず見た目は「一瞬だけ点いて消える」ままになる
    - `main.rs`: `App::update()`冒頭に`if self.steamvr_overlay.is_some() { ctx.request_repaint_after(Duration::from_millis(100)); }`を追加。オーバーレイが有効な間は約100ms間隔で`update()`を強制的に周期実行させ、デスクトップウィンドウの表示/フォーカス状態に関わらず`action_rx`のdrainとスナップショット往復が定期的に走るようにした
    - `cargo test`(30件)/`cargo build`(host)/`cargo build --target x86_64-pc-windows-gnu`(警告ゼロ)/`cargo fmt --check`全て確認済み。`cargo clippy --all-targets -- -D warnings`は直前の続報1修正時のエラー集合と完全一致(新規リグレッション無し)
    - このWSL環境はSteamVR実行不可のため、実際にチェックが押した状態で保持され続けるかは改めて実機でのユーザー確認が必要
- [ ] Stage 4: `WaitFrameSync`によるフレーム同期、ハプティクスなどの仕上げ
