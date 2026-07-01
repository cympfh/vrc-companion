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

## [ ] feat: 翻訳機能

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

## [ ] SteamVR でも動くようにする！

今までどおり GUI は残すが, SteamVR の overlay にも表示できるようにする
設定画面は不要（それはPCのGUIからやる）
各種チェックボックスとか QvPen ボタンだけが並んでれば最高
