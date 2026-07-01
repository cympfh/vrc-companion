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
