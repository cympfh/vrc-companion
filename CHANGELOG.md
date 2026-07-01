# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
