# VRC Companion

VRChat で遊んでるときにあると便利なあれこれ

## 概要

音声入力(音声認識)したテキストを、クリップボードコピー・アクティブウィンドウへの自動入力・VRChat チャットボックスへの送信・AI (Eliza) への送信、から好きな組み合わせで扱える常駐ツールです。

主な機能:

- 🎙 音声認識 (xAI Speech-to-Text による日本語ストリーミング認識)
- 📋 認識結果をクリップボードへ自動コピー
- ⌨️ 認識結果をアクティブウィンドウへ自動入力(貼り付け or 直接入力、Enter 送信の有無も選択可)
- 💬 認識結果を VRChat のチャットボックスへ送信 (OSC)
- 🤖 認識結果を Eliza (対話エージェントサーバー) へ送信し、その返答を VRChat に送信
- 📝 QvPen 呼び出しボタン

## 必要なもの

- [xAI API Key](https://x.ai/) — 音声認識に使用
- VRChat (OSC を有効にしておくこと。ポート `9000` へ送信します)
- (任意) [Eliza](https://github.com/) 互換の対話エージェントサーバー — `/eliza/api/chat` エンドポイントを持つもの

## セットアップ

1. アプリを起動する
2. 右上の「⚙ Settings」を開く
3. xAI API Key を入力して保存する
4. (Eliza を使う場合) Eliza Agent URL を設定する (デフォルト: `http://localhost:9096`)

設定は以下のファイルに保存されます。

- Linux: `~/.config/vrc-companion/config.json`
- Windows: `%APPDATA%\vrc-companion\config.json`
- macOS: `~/Library/Application Support/vrc-companion/config.json`

## 使い方

1. 「⏺ Start」ボタンで録音開始。無音が続くと自動で録音停止して認識結果が確定します
2. 各チェックボックスで結果の送り先を選択
   - 「Auto-input to active window」と「Send to VRChat」は排他(同時には片方だけ有効)
   - 「Send to Eliza」は上記とは独立して有効化できます
3. 「📝 call QvPen」ボタンで QvPen を呼び出せます

## VR の中の表示（SteamVR オーバーレイ）

SteamVR のダッシュボードに「VRC Companion」のタブが出て、録音の開始・停止、送り先の切り替え、QvPen の呼び出しができます。

SteamVR はあとから起動しても大丈夫です（起動したら数秒で自動でつながります）。SteamVR を終了すると、このアプリも一緒に終了します。このアプリが SteamVR を起動することはありません。

表示の大きさは設定ファイル（上の `config.json`）の `steamvr_overlay_width_m` で変えられます（幅をメートルで。既定は `2.4`）。変えたらアプリを起動しなおしてください。

## コントローラーのボタンで操作する

VRChat を遊んでいる最中でも、コントローラーのボタンで録音を始めたりできます（SteamVR のボタン割り当て機能を使います）。

はじめは、左の十字キー下（Steam Frame）、左 A（Index）、左 X（Quest）をすばやく2回押すと録音が始まります。「録音の開始/停止」「QvPen を呼ぶ」は、はじめは割り当てられていません。

割り当てを見たり、好きなボタンに変えたりするとき:

1. VR の中の「VRC Companion」タブ、またはデスクトップの Settings の「キー割り当て変更」を押す
2. SteamVR の割り当て画面で「録音を開始」「録音の開始/停止」「QvPen を呼ぶ」を好きなボタンに割り当てる（1回押し・2回押し・長押しなども選べます）

SteamVR のダッシュボードを開いている間はボタンが効きません。

録音が始まったとき・止まったときは、目の前の少し下に「録音開始」「録音終了」が約2秒出ます（ダッシュボードを閉じていても見えます）。

## ビルド・実行

VRChat は Windows で動くため、本ツールも Windows ネイティブ実行ファイルとしてビルドします。

```sh
make build   # Windows 用リリースビルド (x86_64-pc-windows-gnu)
make test    # テスト実行
make clean   # ビルド成果物を削除
```

生成物: `target/x86_64-pc-windows-gnu/release/vrc-companion.exe`

WSL からは `\\wsl.localhost\<ディストロ名>\...\vrc-companion.exe` として Windows 側からアクセスできます。

詳細は開発者向けに `Makefile` を参照してください。
