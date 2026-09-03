# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project origin

This project is derived from [winh](~/git/winh/) (a sibling Rust/egui voice-transcription app for the same author). vrc-companion re-extracted a minimal subset of winh's features and rebuilt them with TDD. When in doubt about *why* something is structured a certain way, winh is the reference implementation to diff against — but note the STT client here is a **streaming websocket** client, whereas winh's records a full clip and uploads a WAV; do not assume behavior transfers 1:1.

## Commands

```sh
make build   # release build for Windows (x86_64-pc-windows-gnu) — the actual target platform
make install # build, then copy the .exe to ~/apps/vrc-companion
make run     # install, then run the copied .exe (only works if the exe can execute, i.e. on/via Windows)
make test    # cargo test
make fmt     # cargo fmt
make lint    # cargo clippy --all-targets -- -D warnings
make clean   # cargo clean
```

Run a single test: `cargo test <test_name>` (e.g. `cargo test test_auto_input_and_vrchat_are_mutually_exclusive`). Tests are plain `cargo test` — no target flag needed, they run on the host (Linux/WSL), not the Windows cross-target.

VRChat, SteamVR overlay, and the GUI only run meaningfully on Windows. This repo is developed from WSL and cross-compiled; there is no GPU passthrough in this WSL environment, so the GUI cannot be visually verified from `cargo run` directly here — `make build` (or `make install`) and run the `.exe` on Windows (accessible via `\\wsl.localhost\<distro>\...\vrc-companion.exe`, or `~/apps/vrc-companion/vrc-companion.exe` after `make install`) to see the actual UI.

SteamVR overlay / OpenVR FFI / WGL code is `#[cfg(windows)]`. Host `cargo test` / `cargo build` never compiles those files; always also run `cargo build --target x86_64-pc-windows-gnu` when touching `src/steamvr/`.

`make lint` (`clippy -- -D warnings`) currently fails on a pre-existing warning set (dead_code on Windows-gated types in host builds, `too_many_arguments`, `field_reassign_with_default` in tests, etc.). Do not treat that as a regression unless the set grows. New clippy errors in code you touch should still be fixed.

## Architecture

Single-window `eframe`/`egui` immediate-mode app. All mutable state lives in the `App` struct in `src/main.rs`; `App::update()` runs every frame and does two things before drawing: drains any pending channel messages, then renders. There is no separate event loop or message dispatcher — cross-thread communication is entirely channel-based:

- `transcription_receiver` (`tokio::mpsc`): partial/success/error events streamed from the STT websocket task, running on its own `tokio::runtime::Runtime` spawned per recording session (`App::start_streaming_transcription`) and shut down (`rt.shutdown_background()`) once transcription completes or errors.
- `eliza_response_receiver` (`std::sync::mpsc`): Eliza HTTP call runs on a plain spawned thread (`eliza::ElizaClient::send_chat` is blocking `reqwest`), result polled once per frame.
- `translate_response_receiver` (`std::sync::mpsc`): same pattern as Eliza, for `ElizaClient::translate` (`/eliza/api/translate`). Independent endpoint from chat.
- `mute_trigger_receiver` (`std::sync::mpsc`): OSC listener on port 9001 watches `/avatar/parameters/MuteSelf`; a False→True flip within 1s ("くいくい") starts recording, same as the Start button. Logic is `MuteToggleDetector` in `integrations/vrchat.rs`.
- SteamVR overlay (`OverlayHandle`): `snapshot_tx` (App → overlay thread) and `action_rx` (overlay thread → App). See below.

### Data flow per utterance

1. `AudioRecorder` (`src/audio/mod.rs`) captures mic input via `cpal`, downmixes to mono, chunks into ~100ms `Vec<f32>` buffers pushed through an unbounded channel, and independently tracks silence duration + peak amplitude for the UI meter. Recording auto-stops when `is_silent(silence_duration_secs)` goes true (checked every frame while recording), with a 3s grace period after start.
2. `SpeechToTextClient::stream_transcribe` (`src/audio/speech_to_text.rs`) owns the xAI STT websocket (`wss://api.x.ai/v1/stt`) for the session: waits for `transcript.created`, streams PCM16 audio frames as they arrive, sends `audio.done` once the audio channel closes (recording stopped), and forwards `transcript.partial`/`transcript.done` events back as `TranscriptionMessage`.
3. On `TranscriptionMessage::Success`, `App::on_transcription_success` fans the text out to whichever sinks are enabled in `Config`: clipboard (`arboard`), VRChat OSC (`integrations::vrchat`), Eliza chat (`integrations::eliza`, on its own thread), auto-translate (`ElizaClient::translate`, on its own thread), and/or active-window auto-input (`integrations::auto_input`, via `enigo`).
4. If Eliza is enabled and its response later arrives, and `eliza_response_to_vrchat_enabled` is set, the response is sent to VRChat independently of whatever triggered the original message — Eliza's send path is decoupled from the user-message VRChat toggle by design (see TODO.md's "機能変更" entry — this was a deliberate fix, don't re-couple them).
5. If auto-translate is enabled, the original text is still sent to VRChat immediately (gated by `vrchat_enabled`); when the translation arrives, `{original} / {translated}` is sent to VRChat independently of `vrchat_enabled` (same decouple pattern as Eliza).

### Config invariants (`src/config.rs`)

Two independent exclusive pairs — only ever flip them via the exclusive helpers, which force the other off:

- `auto_input_enabled` ↔ `vrchat_enabled` via `enable_auto_input_exclusive()` / `enable_vrchat_exclusive()`
- `eliza_enabled` ↔ `auto_translate_enabled` via `enable_eliza_exclusive()` / `enable_auto_translate_exclusive()`

`eliza_response_to_vrchat_enabled` is independent of `vrchat_enabled`. Config is a flat JSON struct persisted at the OS config dir (`~/.config/vrc-companion/config.json` on Linux) via `Config::load`/`Config::save`; every field has a `#[serde(default = ...)]` so old config files never fail to deserialize when new fields are added — keep that pattern when adding settings.

### SteamVR overlay (`src/steamvr/`)

Desktop GUI stays; a SteamVR **dashboard overlay** mirrors the operational controls (checkboxes, Start/Stop, QvPen, last transcription / Eliza / translation text). Settings stay on the desktop window only.

**Why custom FFI, not the `openvr` / `ovr_overlay` crates:** the official C++ SDK (and crates wrapping it) uses the MSVC vtable ABI, which is incompatible with this project's cross-compile target `x86_64-pc-windows-gnu`. Instead we `libloading`-load `openvr_api.dll` and call `VR_GetGenericInterface("FnTable:IVROverlay_028", ...)` / `"FnTable:IVRApplications_008"` for ABI-safe function-pointer tables. Field order/count is ABI-critical and transcribed verbatim from [openvr_capi.h](https://github.com/ValveSoftware/openvr/blob/master/headers/openvr_capi.h); unused slots are `PlaceholderFn`. Tripwire tests lock table field counts and struct sizes.

**Why custom WGL, not glutin/winit:** `eframe` already owns the process's only `winit::EventLoop`; creating a second one fails on every platform. `GlOverlayRenderer` (`render.rs`) builds a hidden window + WGL context via `windows-sys`, loads GL through `wglGetProcAddress` (fallback: `opengl32.dll` `GetProcAddress`), and paints egui into an FBO via `egui_glow`. WGL contexts have thread affinity — create/render/destroy stay on the overlay thread spawned by `session::start`.

**Do not couple overlay UI state to Config directly.** Overlay thread is given an `OverlaySnapshot` (copy of the flags + last texts + `is_recording`). Clicks become `OverlayAction`s sent back to `App`, which applies them through the same exclusive helpers, `config.save()`s, and `push_steamvr_snapshot()`s. Config remains the single source of truth.

**Input will not work unless `SetOverlayInputMethod(Mouse)` is called** after `ShowOverlay`. Default is `InputMethod_None`; the compositor then never generates `MouseMove` / `MouseButtonDown` / `Up` even if `PollNextOverlayEvent` is wired correctly. Also call `SetOverlayMouseScale` so `VREvent_Mouse_t.x/y` match the render size. Laser-pointer clicks are always mapped to `egui::PointerButton::Primary` (dashboard trigger = click). Overlay mouse Y is **not** inverted (an early invert was wrong on hardware).

**`App::update` must keep ticking while the overlay is alive.** eframe is reactive and otherwise skips `update()` when the desktop window is unfocused (the usual case while the user is in VR). Overlay clicks then appear to "flash and revert" because the overlay thread keeps redrawing the stale snapshot. `if self.steamvr_overlay.is_some() { ctx.request_repaint_after(100ms); }` exists specifically for this — do not remove it.

Pacing is `WaitFrameSync` (compositor frame sync, not a fixed 33ms timer). Successful clicks trigger a short laser-mouse haptic. Overlay init failure (SteamVR not running, DLL missing) is non-fatal — `steamvr::start()` returns `None`, same pattern as the mute listener.

SteamVR auto-launch is registered every successful overlay init via `IVRApplications` (`AddApplicationManifest` + `SetApplicationAutoLaunch`). Manifest JSON is built in `manifest.rs` (host-testable; not Windows-gated). Overlay key and app key share `manifest::APP_KEY` (`"cympfh.vrc_companion"`) — OpenVR requires they match. This is idempotent side-effect, not a user-facing toggle.

The overlay renderer has its **own** `egui::Context` and must load `fonts/NotoSansJP-Regular.ttf` itself (`setup_fonts` in `render.rs`). The desktop window's font setup in `main.rs` does not apply to it; skipping this produces tofu for Japanese.

### Module layout

- `src/audio/` — mic capture + STT client (things that touch raw audio samples)
- `src/integrations/` — everything that talks to an external system by sending already-transcribed text (VRChat OSC, Eliza HTTP, OS-level auto-input/QvPen via `enigo`/`windows-sys`)
- `src/steamvr/` — SteamVR dashboard overlay
  - `bridge.rs` — all OS: `OverlaySnapshot` / `OverlayAction` / `OverlayHandle` / `start()` (non-Windows always `None`) / `overlay_fields()`
  - `ffi.rs` — `#[cfg(windows)]`: raw OpenVR C API tables
  - `session.rs` — `#[cfg(windows)]`: overlay lifecycle, event poll, texture submit, auto-launch register
  - `render.rs` — `#[cfg(windows)]`: WGL + `egui_glow` offscreen painter
  - `manifest.rs` — all OS: `.vrmanifest` JSON (host-testable)
- `src/config.rs`, `src/main.rs` — stay at the root

`auto_input::call_qvpen` and the Windows-`FindWindowW`/`SetForegroundWindow` calls in `integrations/auto_input.rs` are `#[cfg(windows)]`-gated; the non-Windows path just returns an error, since this only matters on the deployed target.

Windows-gated types/fns that are only constructed from `#[cfg(windows)]` code (`OverlayAction` variants, `write_manifest`, …) will trip `dead_code` on host builds. That is expected; `#[allow(dead_code)]` at those sites is the established pattern, not leftover junk. Do not "clean them up" by deleting the allow or the type.

## Process notes

- Development follows TDD (see TODO.md) — tests are colocated with the code they cover (`#[cfg(test)] mod tests` at the bottom of each file), not in a separate test tree.
- TODO.md is the running dev log for this project (checkboxes with completion timestamps + notes on what was actually done/decided) — check it for the rationale behind recent changes before assuming something is unfinished or accidental. SteamVR Stage 1–4 plus overlay UI follow-ups and the AFK-toggle revert live there.
- AFK toggle (`/input/AFKToggle`) was implemented, found unsupported on hardware, and **fully removed** on user instruction. Do not re-add it.
