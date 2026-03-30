# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development

```bash
cargo build                                    # Debug build
cargo run                                      # Run (info level logging)
RUST_LOG=open_sound_grid=debug cargo run       # Debug logging
RUST_LOG=open_sound_grid::plugins=trace cargo run  # Trace PA plugin only

cargo test                                     # 84 unit + 20 journey tests (no PA required)
cargo test --test journey_tests -- --ignored   # Run all 85 journey tests (incl. aspirational)
cargo test --test ui_tests -- --ignored        # 12 UI snapshot tests (iced_test headless)
cargo fmt --all                                # Format (enforced by CI)
cargo clippy -- -D warnings                   # Lint, zero warnings required

cargo build --release                          # Release build (LTO, stripped)
```

**System deps**: `libpulse-dev`, `libdbus-1-dev`, `pkg-config`
**Rust version**: 1.85+ (edition 2024)
**CI**: fmt → clippy (`-D warnings`) → tests → release build

## Architecture

Layered message-passing design. UI never touches plugin internals directly.

```
UI (iced) → App (state + message dispatch) → MixerEngine → Plugin (AudioPlugin trait) → PulseAudio
                                                    ↑ PluginEvent (async subscription)
```

**Audio model**: Sources route to mixes via PA loopback modules. Each cell in the matrix = one loopback with independent volume control via sink-input.

### Key layers

- **`src/app/`** — Application core: `state.rs` (all UI + config state), `messages.rs` (100+ Message variants), `update.rs` (dispatch router), `handlers/` (12 handler modules by responsibility)
- **`src/engine/`** — `MixerEngine` sends `PluginCommand`s, holds `MixerState` (UI-facing mirror of plugin state)
- **`src/plugin/`** — `AudioPlugin` trait + `PluginCommand`/`PluginEvent` protocol in `api.rs`. Plugin runs on dedicated thread via `manager.rs`
- **`src/plugins/pulseaudio/`** — PA backend (15 modules): null sinks, loopbacks, volume, peaks, app discovery, device enumeration. All native libpulse introspect (no pactl shell-outs in audio path)
- **`src/plugins/pipewire/`** — PipeWire backend skeleton (disabled by default, not functional)
- **`src/ui/`** — Widgets: `matrix/` (grid, cell, headers), `eq/` (canvas biquad + spectrum), `vu_slider.rs` (VU-as-slider-track merged widget), effects panel, sidebar, app list. Theme tokens in `theme.rs`

### Other modules

- **`src/autostart.rs`** — XDG .desktop autostart install/remove
- **`src/sound_check.rs`** — Record/playback loop for mic testing (state machine: Idle → Recording → Ready → Playing)
- **`src/hotkeys.rs`** — Global hotkey via KDE kglobalacceld (Ctrl+Shift+M → mute all)
- **`src/notifications.rs`** — Desktop notifications for device changes
- **`src/presets.rs`** — Preset save/load/list/delete (TOML files in ~/.config/open-sound-grid/presets/)
- **`src/resolve.rs`** — XDG desktop entry icon resolution (freedesktop_icons)
- **`src/tray.rs`** — System tray via ksni (Show/Mute All/Quit)

### Critical patterns

- **Deferred state restoration** (`persistence.rs`, `state_restore.rs`): Config restores channels/mixes immediately but defers routes, devices, and effects until first `PluginEvent::StateRefreshed` (PA IDs unknown until resources are created)
- **Lock-free peak levels**: `AtomicU32` shared between PA background thread and UI — no mutex in render path
- **Single-instance guard** (`main.rs`): Detects orphaned lock holders and cleans stale processes before retry
- **WL3 volume model**: Channel master = ceiling, cells = ratio of master. `effective_pa_volume = cell_ratio × channel_master`. Master movement preserves ratios.
- **VU-as-slider-track**: `VuSliderProgram` canvas renders green/amber/red VU fill as slider track background (Wave Link 3.0 signature)
- **Effects architecture**: Channels are pure signal (volume only). Effects (EQ, compressor, gate) belong on mixes — each mix processes each channel independently. Current code has per-channel effects as a convenience; the target model is per-mix effects. Future: input pre-processing (gate, de-essing) on a separate inputs page.

### PulseAudio resource tracking

The PA plugin maintains bidirectional maps: `channel_id ↔ sink name`, `(source, mix) ↔ loopback module ID`, `(source, mix) ↔ sink-input index`. All module load/unload tracked for clean shutdown.

## Tests

- **Unit tests** (84): Config serialization, default values, channel/mix lifecycle, route state, PA module parsing, widget behavior
- **Journey tests** (101 total, 20 non-ignored): TDD-style aspirational tests mapping to 12 user journeys from journey-spec.md. `#[ignore]` tests encode dream behavior — failing = gaps to fix, not bugs.
- **UI snapshot tests** (12, all ignored): Headless layout verification via iced_test + SHA256 golden hashes in `tests/snapshots/`
- **Volume model tests**: WL3 ratio model, channel master persistence, stereo L/R, perceptual curve

## Feature flags

```toml
pipewire-backend = ["dep:pipewire", "dep:libspa"]  # Optional, not default
```

## Config

Runtime config persisted via `confy` to `~/.config/open-sound-grid/default-config.toml`. Serialization types in `src/config.rs`. Contains channels, mixes, routes, audio settings, UI preferences, device failover list, seen apps, presets.

## Volume control

PipeWire stereo volume requires `cv.set(2, vol)` — using `set(1)` silently fails (sets only left channel). Always set both channels explicitly.
