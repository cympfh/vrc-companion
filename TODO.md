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

## [ ] feat

### 機能追加

- Eliza の返答をGUIに表示する
    - speech-to-text の結果の下に同様に表示する
- Eliza からの返答を VRChat に送信するかどうか選べるチェックボックス

### 機能変更

Eliza に送信チェックを入れてても、右手の hand gesture を追加しないと実際には送信してないのが現状

チェックが入ってたら全部送ることにする。

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
