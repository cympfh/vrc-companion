# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project origin

This project is derived from [winh](~/git/winh/) (a sibling Rust/egui voice-transcription app for the same author). vrc-companion re-extracted a minimal subset of winh's features and rebuilt them with TDD. When in doubt about *why* something is structured a certain way, winh is the reference implementation to diff against — but note the STT client here is a **streaming websocket** client, whereas winh's records a full clip and uploads a WAV; do not assume behavior transfers 1:1.

## Commands

```sh
make build   # release build for Windows (x86_64-pc-windows-gnu) — the actual target platform
make run     # build then run the .exe (only works if the exe can execute, i.e. on/via Windows)
make test    # cargo test
make fmt     # cargo fmt
make lint    # cargo clippy --all-targets -- -D warnings
make clean   # cargo clean
```

Run a single test: `cargo test <test_name>` (e.g. `cargo test test_auto_input_and_vrchat_are_mutually_exclusive`). Tests are plain `cargo test` — no target flag needed, they run on the host (Linux/WSL), not the Windows cross-target.

VRChat and the GUI only run meaningfully on Windows. This repo is developed from WSL and cross-compiled; there is no GPU passthrough in this WSL environment, so the GUI cannot be visually verified from `cargo run` directly here — build for Windows and run the `.exe` (accessible from Windows via `\\wsl.localhost\<distro>\...\vrc-companion.exe`) to see the actual UI.

## Architecture

Single-window `eframe`/`egui` immediate-mode app. All mutable state lives in the `App` struct in `src/main.rs`; `App::update()` runs every frame and does two things before drawing: drains any pending channel messages, then renders. There is no separate event loop or message dispatcher — cross-thread communication is entirely channel-based:

- `transcription_receiver` (`tokio::mpsc`): partial/success/error events streamed from the STT websocket task, running on its own `tokio::runtime::Runtime` spawned per recording session (`App::start_streaming_transcription`) and shut down (`rt.shutdown_background()`) once transcription completes or errors.
- `eliza_response_receiver` (`std::sync::mpsc`): Eliza HTTP call runs on a plain spawned thread (`eliza::ElizaClient::send_chat` is blocking `reqwest`), result polled once per frame.

### Data flow per utterance

1. `AudioRecorder` (`src/audio/mod.rs`) captures mic input via `cpal`, downmixes to mono, chunks into ~100ms `Vec<f32>` buffers pushed through an unbounded channel, and independently tracks silence duration + peak amplitude for the UI meter. Recording auto-stops when `is_silent(silence_duration_secs)` goes true (checked every frame while recording), with a 3s grace period after start.
2. `SpeechToTextClient::stream_transcribe` (`src/audio/speech_to_text.rs`) owns the xAI STT websocket (`wss://api.x.ai/v1/stt`) for the session: waits for `transcript.created`, streams PCM16 audio frames as they arrive, sends `audio.done` once the audio channel closes (recording stopped), and forwards `transcript.partial`/`transcript.done` events back as `TranscriptionMessage`.
3. On `TranscriptionMessage::Success`, `App::on_transcription_success` fans the text out to whichever sinks are enabled in `Config`: clipboard (`arboard`), VRChat OSC (`integrations::vrchat`), Eliza (`integrations::eliza`, on its own thread), and/or active-window auto-input (`integrations::auto_input`, via `enigo`).
4. If Eliza is enabled and its response later arrives, and `eliza_response_to_vrchat_enabled` is set, the response is sent to VRChat independently of whatever triggered the original message — Eliza's send path is decoupled from the user-message VRChat toggle by design (see TODO.md's "機能変更" entry — this was a deliberate fix, don't re-couple them).

### Config invariants (`src/config.rs`)

`auto_input_enabled` and `vrchat_enabled` are mutually exclusive by construction — only ever flip them via `enable_auto_input_exclusive()` / `enable_vrchat_exclusive()`, which force the other off. `eliza_enabled` is independent of both. Config is a flat JSON struct persisted at the OS config dir (`~/.config/vrc-companion/config.json` on Linux) via `Config::load`/`Config::save`; every field has a `#[serde(default = ...)]` so old config files never fail to deserialize when new fields are added — keep that pattern when adding settings.

### Module layout

- `src/audio/` — mic capture + STT client (things that touch raw audio samples)
- `src/integrations/` — everything that talks to an external system by sending already-transcribed text (VRChat OSC, Eliza HTTP, OS-level auto-input/QvPen via `enigo`/`windows-sys`)
- `src/config.rs`, `src/main.rs` — stay at the root

`auto_input::call_qvpen` and the Windows-`FindWindowW`/`SetForegroundWindow` calls in `integrations/auto_input.rs` are `#[cfg(windows)]`-gated; the non-Windows path just returns an error, since this only matters on the deployed target.

## Process notes

- Development follows TDD (see TODO.md) — tests are colocated with the code they cover (`#[cfg(test)] mod tests` at the bottom of each file), not in a separate test tree.
- TODO.md is the running dev log for this project (checkboxes with completion timestamps + notes on what was actually done/decided) — check it for the rationale behind recent changes before assuming something is unfinished or accidental.
