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
    pulseaudio/    — PulseAudio backend (null sinks + loopback routing)
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

- **No shared state between plugin and UI** — all communication flows through typed `PluginCommand` / `PluginEvent` messages over async channels. The UI never touches plugin internals directly.
- **Zero-latency event subscription** — plugin events arrive through an `iced::Subscription` stream, not polling. Peak level updates drive VU meters at ~20ms intervals without blocking the UI thread.
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
| Application routing panel | Done | Route any running app to any channel |
| App name resolution | Done | freedesktop desktop entry lookup |
| Hardware output selection | Done | Per-mix output device picker |
| Live VU meters | Done | Driven by async peak level events |
| Config persistence | Done | TOML via confy, auto-saved on change |
| Config restore on launch | Done | Channels and mixes recreated at startup |
| System tray | Done | ksni SNI tray: Show, Mute All, Quit |
| Single-instance guard | Done | Second launch focuses existing window |
| Collapsible sidebar | Done | Hardware input panel, toggle button |
| Connection status indicator | Done | Live dot + text in status bar |
| Hardware input sidebar | Done | Lists physical inputs with VU meters |
| Dark theme | Done | Custom design token system |
| PipeWire native backend | Planned | v0.3 target |
| Per-mix effects (EQ, compression) | Planned | v0.2 target |
| JACK backend | Planned | v0.3 target |
| VST3 / LV2 plugin hosting | Future | Post-v0.3 |
| Mobile companion app | Future | Remote control via local network |

## Keyboard Shortcuts

| Shortcut | Action | Status |
|----------|--------|--------|
| `Ctrl+M` | Mute all channels | Planned |
| `Ctrl+,` | Open settings | Planned |
| `Ctrl+W` | Hide to tray | Planned |
| `Ctrl+Q` | Quit | Planned |
| `Ctrl+N` | New channel | Planned |
| `Ctrl+Shift+N` | New mix | Planned |
| `Tab` | Cycle focus through matrix cells | Planned |
| `Space` | Toggle selected cell enable/disable | Planned |
| `Up/Down` | Adjust selected cell volume ±5% | Planned |
| `Shift+Up/Down` | Adjust selected cell volume ±1% | Planned |

Keyboard navigation is planned for v0.2. Currently all interaction is mouse-driven.

## Configuration

OpenSoundGrid stores its config at `~/.config/open-sound-grid/default-config.toml` (managed by `confy`). The file is created on first launch and auto-saved whenever channels or mixes change.

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
icon = ""
color = [100, 149, 237]
output_device = "alsa_output.pci-0000_00_1f.3.analog-stereo"

[[mixes]]
name = "Stream"
icon = ""
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
| `mixes[].color` | RGB accent color `[r, g, b]` | `[128, 128, 128]` |
| `mixes[].output_device` | PulseAudio sink name for this mix | `""` (auto) |
| `audio.latency_ms` | Loopback latency in milliseconds | `20` |
| `audio.output_device` | Default output device | `"auto"` |
| `ui.window_width` / `ui.window_height` | Window dimensions | `1000 x 600` |
| `ui.compact_mode` | Compact layout toggle | `false` |

To reset to defaults, delete the config file and relaunch:

```bash
rm ~/.config/open-sound-grid/default-config.toml
```

## Roadmap

| Version | Focus | Status |
|---------|-------|--------|
| **v0.1** | Matrix mixer core | Current |
| **v0.2** | Effects, keyboard navigation, polish | Planned |
| **v0.3** | PipeWire native backend, JACK support | Future |
| **v1.0** | Stable API, packaging, full docs | Future |

### v0.1 — Matrix Mixer Core (current)

- PulseAudio null sink channels + loopback routing
- Full matrix grid with per-cell volume and enable/disable
- Application routing panel with freedesktop name resolution
- Config persistence and restore on launch
- System tray (ksni SNI), single-instance guard
- Live VU meters via async peak level events
- Hardware output selection per mix

### v0.2 — Effects and Polish (planned)

- Per-channel EQ (parametric, 3-band)
- Noise gate and compression on voice channels
- Keyboard navigation throughout the matrix
- Drag-and-drop column/row reordering
- Per-mix color themes
- Improved onboarding (first-run setup wizard)

### v0.3 — PipeWire Native and Integrations (future)

- PipeWire native backend (replaces loopback hacks with filter-chain nodes)
- JACK backend
- D-Bus control interface (scriptable from external tools)
- OBS integration (scene-triggered mix presets)
- Streaming deck button mapping

## Contributing

```bash
cargo check                    # Type-check without building
cargo build                    # Debug build
cargo test                     # Run all tests
cargo fmt --all                # Format (required before commit)
cargo clippy -- -D warnings    # Lint (must pass clean)
```

PulseAudio must be running when building or running integration tests. Set `RUST_LOG=debug` for verbose tracing output:

```bash
RUST_LOG=open_sound_grid=debug cargo run
```

All changes go through a pull request. Follow [conventional commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `refactor:`, `chore:`, etc.

## License

[CC BY-NC-SA 4.0](LICENSE.md) — free for non-commercial use. Commercial use requires a separate license.

<p align="center">
  <br />
  <sub>Built with Rust + iced. Runs on PulseAudio. Designed for Linux creators.</sub>
</p>
