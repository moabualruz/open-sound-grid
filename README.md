<p align="center">
  <strong>OpenSoundGrid</strong>
</p>

<p align="center">
  <strong>Professional audio matrix routing for Linux creators.</strong>
</p>

<p align="center">
  <a href="https://github.com/moabualruz/open-sound-grid/actions"><img src="https://img.shields.io/github/actions/workflow/status/moabualruz/open-sound-grid/ci.yml?branch=main&style=flat-square&label=CI" alt="CI Status" /></a>
  <a href="https://github.com/moabualruz/open-sound-grid/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/license-CC%20BY--NC--SA%204.0-00a020?style=flat-square" alt="License" /></a>
  <a href="https://github.com/moabualruz/open-sound-grid/releases"><img src="https://img.shields.io/github/v/release/moabualruz/open-sound-grid?style=flat-square&color=00a020" alt="Latest Release" /></a>
</p>

---

**OpenSoundGrid** is a native Linux audio matrix mixer. It gives streamers, podcasters, and musicians the same per-source, per-mix volume control that Wave Link and GoXLR software provide on other platforms — built on PulseAudio today, with PipeWire native support on the roadmap.

## Table of Contents

- [Why OpenSoundGrid](#why-opensoundgrid)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [How It Works](#how-it-works)
- [Architecture](#architecture)
- [Features](#features)
- [Keyboard Shortcuts](#keyboard-shortcuts)
- [Configuration](#configuration)
- [Tests](#tests)
- [Known Limitations](#known-limitations-v02)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License](#license)

## Why OpenSoundGrid

Linux has no Wave Link equivalent. Every tool that exists is either too technical for non-engineers or too limited for real production use.

- **PulseAudio module-loopback routing** requires manual `pactl` commands and breaks on daemon restart. OpenSoundGrid manages it automatically.
- **pavucontrol** shows streams but has no matrix, no per-destination volumes, and no persistent routing. OpenSoundGrid persists everything across sessions.
- **JACK** is powerful but requires a complete audio stack replacement and deep technical knowledge. OpenSoundGrid works on top of your existing PulseAudio setup.
- **Existing mixers target hardware** — GoXLR, Rodecaster, Wave Link all require proprietary hardware. OpenSoundGrid is software-only.
- **No app routing** — nothing on Linux lets you send specific applications to specific mixes (e.g., Discord to Monitor only, game audio to Stream + Monitor). OpenSoundGrid does this with a single pick list.

OpenSoundGrid fixes all of this with a single native binary, zero daemon changes, and a purpose-built matrix UI.

## Quick Start

```bash
git clone https://github.com/moabualruz/open-sound-grid.git
cd open-sound-grid
cargo run
```

PulseAudio must be running. The app auto-discovers hardware inputs/outputs and running audio applications on startup.

```bash
# Run with full debug tracing
RUST_LOG=open_sound_grid=debug cargo run

# Run with trace-level (very verbose)
RUST_LOG=open_sound_grid=trace cargo run
```

## Installation

### From Source (Rust)

```bash
git clone https://github.com/moabualruz/open-sound-grid.git
cd open-sound-grid
cargo build --release
./target/release/open-sound-grid
```

### AUR (Arch Linux)

```bash
yay -S open-sound-grid
# or
paru -S open-sound-grid
```

### System Requirements

| Requirement | Version |
|-------------|---------|
| Rust | 1.85+ (edition 2024) |
| PulseAudio | 15+ |
| Linux | Any desktop distribution |

## How It Works

OpenSoundGrid models audio as a **matrix**: sources on the left, mixes across the top. Every intersection is an independent volume control backed by a PulseAudio `module-loopback` instance.

```
                  ┌───────────┬───────────┬───────────┐
                  │  Monitor  │  Stream   │  Podcast  │
  ┌───────────────┼───────────┼───────────┼───────────┤
  │  Music        │  100%  ✓  │   80%  ✓  │    0%  ✗  │
  ├───────────────┼───────────┼───────────┼───────────┤
  │  Game         │  100%  ✓  │   65%  ✓  │    0%  ✗  │
  ├───────────────┼───────────┼───────────┼───────────┤
  │  Voice (mic)  │   85%  ✓  │  100%  ✓  │  100%  ✓  │
  ├───────────────┼───────────┼───────────┼───────────┤
  │  System       │   70%  ✓  │    0%  ✗  │    0%  ✗  │
  └───────────────┴───────────┴───────────┴───────────┘
         ↑                ↑
    null sinks       loopback modules
   (channels)        (matrix cells)
```

**Channels** are PulseAudio null sinks. Applications are routed to a channel by moving their stream. **Mixes** are output destinations — each backed by a set of module-loopbacks, one per active channel. Enabling a cell in the matrix creates (or activates) the corresponding loopback; adjusting its volume sets the loopback latency-corrected volume.

Configuration is persisted to TOML via `confy` and restored on next launch. No PulseAudio config files are modified.

## Architecture

```
src/
  app.rs           — iced Application: Message enum, update(), view(), subscriptions
  main.rs          — entry point: single-instance guard, plugin init, tray, iced startup
  config.rs        — TOML persistence via confy (~/.config/open-sound-grid/)
  resolve.rs       — freedesktop app name/icon resolution from desktop entries
  tray.rs          — system tray via ksni (StatusNotifierItem / SNI protocol)
  error.rs         — thiserror error types
  lib.rs           — crate root, module declarations
  engine/
    mod.rs         — MixerEngine: command sender, connection state, plugin bridge
    state.rs       — MixerState: UI-facing mirror of plugin state (channels, mixes, routes)
  plugin/
    mod.rs         — AudioPlugin trait definition, capabilities, plugin info
    api.rs         — PluginCommand / PluginResponse / PluginEvent protocol (no shared state)
    manager.rs     — event-driven plugin thread, unified command/event channel
  plugins/
    pulseaudio/
      mod.rs       — PulseAudio backend: channel/mix lifecycle, loopback routing
      connection.rs— PA mainloop connection wrapper
      apps.rs      — running application stream discovery and routing
      devices.rs   — hardware sink/source enumeration, output device selection
      modules.rs   — null sink and loopback module management
      peaks.rs     — pactl subscribe listener for real-time peak level events
  ui/
    matrix.rs      — matrix grid widget: the core routing surface
    sidebar.rs     — collapsible hardware input sidebar
    app_list.rs    — detected application routing panel
    audio_slider.rs— volume slider with dB readout
    vu_meter.rs    — horizontal VU meter bar (driven by peak level events)
    theme.rs       — design tokens (colors, spacing)
    mod.rs         — UI module root
```

**Key design decisions:**

- **Unified channel architecture** — channels and mixes are both first-class objects in `MixerState`. Every route cell (channel × mix) carries its own volume and mute state. Add or remove channels/mixes at runtime; the matrix redraws and PA state updates atomically.
- **No shared state between plugin and UI** — all communication flows through typed `PluginCommand` / `PluginEvent` messages over async channels. The UI never touches plugin internals directly.
- **Event-driven plugin thread** — the PA backend runs a dedicated event loop using `pactl subscribe` for real-time change notifications. There is no polling loop; events wake the thread only when PA signals a change. The thread owns the PA mainloop connection for its lifetime.
- **Zero-latency event subscription** — plugin events arrive through an `iced::Subscription` stream, not polling. Peak level updates drive VU meters at ~20ms intervals without blocking the UI thread.
- **libpulse introspect migration (partial)** — device listing (`get_sink_info_list`, `get_source_info_list`) and application stream discovery (`get_sink_input_info_list`) use the native libpulse introspect API. Module load/unload, volume control, mute, and stream move operations still shell out to `pactl`. Full migration is planned for v0.3.
- **Plugin trait abstraction** — the `AudioPlugin` trait decouples the UI from PulseAudio. A PipeWire backend can be added without touching the matrix, engine, or UI code.
- **Single binary** — no daemon, no background service. The app owns its PulseAudio connection for its lifetime.

## Features

| Feature | Status | Notes |
|---------|--------|-------|
| Matrix routing grid | Done | Sources x Mixes, per-cell volume sliders |
| PulseAudio null sink channels | Done | Created and managed automatically |
| PulseAudio loopback routing | Done | One loopback per active matrix cell |
| Per-cell volume control | Done | 0–100%, persisted across sessions |
| Per-cell enable/disable toggle | Done | Enables/disables the loopback module |
| Mix master volume | Done | Scales all loopbacks in the mix |
| Mix mute | Done | Mutes the mix output |
| Source mute | Done | Mutes a channel across all mixes |
| Per-route mute | Done | Mute individual channel→mix routes independently |
| Add / remove channel | Done | Runtime add and remove with PA cleanup |
| Add / remove mix | Done | Runtime add and remove with PA cleanup |
| Application routing panel | Done | Route any running app to any channel |
| App name resolution | Done | freedesktop desktop entry lookup, locale-aware |
| Per-mix output device selection | Done | Pick any PA sink per mix at runtime |
| Output device restore on startup | Done | Saved per-mix device reapplied on launch |
| Settings panel | Done | Basic settings panel (compact mode toggle) |
| Compact mode persistence | Done | compact_mode persisted to TOML |
| Live VU meters | Done | Volume-based; real signal peaks via PA_STREAM_PEAK_DETECT — v0.3 |
| Config persistence | Done | TOML via confy, auto-saved on change |
| Config restore on launch | Done | Channels and mixes recreated at startup |
| System tray | Done | ksni SNI tray: Show, Mute All, Quit |
| Single-instance guard | Done | Second launch focuses existing window |
| Collapsible sidebar | Done | Hardware input panel, toggle button |
| Connection status indicator | Done | Live dot + text in status bar |
| Hardware input sidebar | Done | Lists physical inputs with VU meters |
| Full tracing instrumentation | Done | `tracing` spans + fields on every code path |
| Dark theme | Done | Custom design token system |
| Unit test suite | Done | 40 unit tests, zero clippy warnings |
| PipeWire native backend | Planned | v0.3 target |
| Per-mix effects (EQ, compression) | Done | Parameter UI + storage; inline audio processing requires PA stream capture — v0.3 |
| JACK backend | Planned | v0.3 target |
| VST3 / LV2 plugin hosting | Future | Post-v0.3 |
| Mobile companion app | Future | Remote control via local network |

## Keyboard Shortcuts

| Shortcut | Action | Status |
|----------|--------|--------|
| `Ctrl+M` | Mute all channels | Done |
| `Ctrl+,` | Open settings | Done |
| `Ctrl+W` | Hide to tray | Done |
| `Ctrl+Q` | Quit | Done |
| `Ctrl+N` | New channel | Done |
| `Ctrl+Shift+N` | New mix | Done |
| `Tab` | Cycle focus through matrix cells | Done |
| `Space` | Toggle selected cell enable/disable | Done |
| `Up/Down` | Adjust selected cell volume ±5% | Done |
| `Shift+Up/Down` | Adjust selected cell volume ±1% | Done |

## Configuration

OpenSoundGrid stores its config at `~/.config/open-sound-grid/default-config.toml` (managed by `confy`). The file is created on first launch and auto-saved whenever channels, mixes, or UI state change.

```toml
[[channels]]
name = "Music"

[[channels]]
name = "Game"

[[channels]]
name = "Voice"

[[channels]]
name = "System"

[[mixes]]
name = "Monitor"
icon = "🎧"
color = [100, 149, 237]
output_device = "alsa_output.pci-0000_00_1f.3.analog-stereo"

[[mixes]]
name = "Stream"
icon = "📡"
color = [255, 99, 71]
output_device = ""

[audio]
latency_ms = 20
output_device = "auto"

[ui]
compact_mode = false
window_width = 1000
window_height = 600
```

### Configuration Fields

| Field | Description | Default |
|-------|-------------|---------|
| `channels[].name` | Display name for the channel (null sink) | — |
| `mixes[].name` | Display name for the output mix | — |
| `mixes[].icon` | Emoji or single character shown in the mix header | `""` |
| `mixes[].color` | RGB accent color `[r, g, b]` | `[128, 128, 128]` |
| `mixes[].output_device` | PulseAudio sink name for this mix; omit or `""` for auto | `null` (auto) |
| `audio.latency_ms` | Loopback latency in milliseconds | `20` |
| `audio.output_device` | Default fallback output device | `"auto"` |
| `ui.window_width` / `ui.window_height` | Window dimensions | `1000 x 600` |
| `ui.compact_mode` | Compact layout toggle | `false` |

To reset to defaults, delete the config file and relaunch:

```bash
rm ~/.config/open-sound-grid/default-config.toml
```

## Tests

```bash
cargo test           # 48 unit tests
cargo clippy         # zero warnings
```

Tests cover config serialization/deserialization, default values, channel/mix lifecycle, route state mutations, and PA module name parsing. PulseAudio does not need to be running to execute the unit test suite.

## Known Limitations (v0.2)

- **VU meters show configured volume, not signal amplitude.** True peak monitoring
  via PA's PEAK_DETECT stream flag is planned for v0.3.
- **Effects chain is parameter-only.** The UI controls and parameter persistence work,
  but audio is not actually processed through the effects graph. Inline processing
  requires PA stream capture, planned for v0.3.
- **Light theme partially applied.** Custom widget styles use theme-aware colors but
  some iced default widgets may not fully match the warm palette.
- **Module operations use pactl CLI.** Device listing and app detection use the native
  libpulse introspect API, but module load/unload, volume control, and event
  subscription still shell out to pactl. Full migration planned for v0.3.

## Roadmap

| Version | Focus | Status |
|---------|-------|--------|
| **v0.1** | Matrix mixer core | Done |
| **v0.2** | Effects, keyboard navigation, polish | Done (with noted limitations) |
| **v0.3** | PipeWire native backend, JACK support, real peaks, full libpulse migration | Planned |
| **v1.0** | Stable API, packaging, full docs | Future |

### v0.1 — Matrix Mixer Core (done)

- PulseAudio null sink channels + loopback routing — Done
- Full matrix grid with per-cell volume and enable/disable — Done
- Application routing panel with freedesktop name resolution — Done
- Config persistence and restore on launch — Done
- System tray (ksni SNI), single-instance guard — Done
- Live VU meters via async peak level events — Done
- Per-mix output device selection and restore on startup — Done
- Add / remove channels and mixes at runtime — Done
- Per-route mute (independent of channel and mix mute) — Done
- Settings panel with compact mode persistence — Done
- Full tracing instrumentation across all code paths — Done

### v0.2 — Effects and Polish (done, with noted limitations)

- Per-channel effects chain: EQ, compressor, noise gate — parameter UI and storage done
- Keyboard navigation throughout the matrix — Done
- Dark/light theme toggle with warm palette — Done
- Full tracing instrumentation — Done
- Presets (save/load named mixer state) — Done
- _Limitation:_ effects audio processing and real peak monitoring deferred to v0.3 (see Known Limitations)

### v0.3 — PipeWire Native and Integrations (planned)

- PipeWire native backend (replaces loopback hacks with filter-chain nodes)
- JACK backend
- Real signal peak monitoring via PA streams (PA_STREAM_PEAK_DETECT)
- Inline effects audio processing (fundsp, PA stream capture)
- Full pactl→libpulse migration for write ops
- D-Bus control interface (scriptable from external tools)
- OBS integration (scene-triggered mix presets)
- Streaming deck button mapping

## Contributing

```bash
cargo check                    # Type-check without building
cargo build                    # Debug build
cargo test                     # Run all unit tests (no PA required)
cargo fmt --all                # Format (required before commit)
cargo clippy -- -D warnings    # Lint (must pass clean)
```

PulseAudio must be running for integration and end-to-end testing. The unit test suite runs without PA. Set `RUST_LOG` for verbose tracing output when debugging:

```bash
RUST_LOG=open_sound_grid=debug cargo run
RUST_LOG=open_sound_grid=trace cargo run
```

All changes go through a pull request. Follow [conventional commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `refactor:`, `chore:`, etc.

## License

[CC BY-NC-SA 4.0](LICENSE.md) — free for non-commercial use. Commercial use requires a separate license.

<p align="center">
  <br />
  <sub>Built with Rust + iced. Runs on PulseAudio. Designed for Linux creators.</sub>
</p>
