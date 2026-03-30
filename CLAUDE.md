# CLAUDE.md

## Build & Development

```bash
cargo check                    # Type check workspace
cargo test                     # All tests (unit + integration)
cargo fmt --all                # Format
cargo clippy -- -D warnings    # Lint, zero warnings required

cargo run -p osg-server        # Run web server
RUST_LOG=osg_core=debug cargo run -p osg-server  # Debug logging
```

**System deps**: `libpipewire-0.3-dev`, `libclang-dev`, `pkg-config`
**Rust**: 1.94+ (edition 2024)
**CI**: fmt → clippy (`-D warnings`) → tests. Run locally before every commit. GitHub Actions exist but are manual-trigger only.

## Architecture

UI/UX layer over PipeWire. OSG does not reinvent audio infrastructure — it configures, monitors, and controls existing tools via their native APIs.

```
Phone/Browser → Web UI (SolidJS + Tailwind v4)
                    ↕ WebSocket (per-service-category connections)
                Rust Backend (osg-server, Axum)
                    ↕ service traits
                osg-core
                    ├── MixerSession (write model — user's desired state)
                    ├── AudioGraph (read model — PipeWire reality)
                    ├── ReconciliationService (stateless diff → corrective events)
                    ├── EventBus (typed channels per command category)
                    └── Handlers (infrastructure: PW API, config, EasyEffects socket)
                         ↕ pipewire-rs 0.9
                    PipeWire daemon
```

### Crate boundaries (ADR-003)

- **`osg-core`** — PipeWire orchestration, domain model, state, config. Zero knowledge of HTTP/WebSocket/UI.
- **`osg-server`** — Depends on osg-core. Axum + WebSocket. Serves web UI static files.
- **`osg-desktop`** (future) — Depends on osg-core, NOT osg-server. Native UI, calls core directly.
- **`poc/`** — Original iced app. Reference only, not part of the workspace build.

### Domain model (DDD)

- **MixerSession** (aggregate root, write model): Endpoints, GroupNodes, Routes, Locks, Applications, Presets. Only mutated by user commands. Every `handle(command)` returns a new immutable snapshot + domain events. Published via `tokio::sync::watch`.
- **AudioGraph** (read model): Projection of PipeWire reality. Only mutated by PipeWire registry events. Never written to by user actions.
- **ReconciliationService** (domain service, stateless): Reads MixerSession + AudioGraph, emits corrective events when desired state diverges from PipeWire reality. No state of its own.
- **Handlers** (infrastructure): Translate domain events into PipeWire API calls, disk writes, WebSocket messages. No business logic. One handler per command category.

### Event-driven (separate pipes)

Each command category has its own typed channel with independent backpressure and debounce:

| Channel | Frequency | Debounce | Handler |
|---------|-----------|----------|---------|
| VolumeCommands | High (60Hz slider) | 16ms | VolumeHandler → PW `set_param` |
| LinkCommands | Medium | None | LinkHandler → PW `create_object`/`destroy` |
| NodeCommands | Low | None | NodeHandler → PW adapter factory |
| MetadataCommands | Low | None | MetadataHandler → PW metadata API |
| ConfigEvents | Low | 30s | ConfigHandler → TOML disk |
| WebSocketBroadcast | All events | None | Subscribes to all channels → broadcasts to UI |

User events and correction events use the same types and flow through the same handlers. The handler does not know or care about the origin.

### Transport-agnostic services

Each command category is a trait. Transport is a pluggable adapter:

```rust
trait VolumeService {
    fn set_volume(&self, node: NodeId, left: f32, right: f32) -> Result<(), VolumeError>;
    fn subscribe(&self) -> Receiver<VolumeEvent>;
}
```

- Web UI → WebSocket adapters (one WS connection per service category)
- Desktop app → stdio JSON-RPC adapters (future)
- CLI → same JSON-RPC over stdin/stdout (future)
- REST → polling fallback for simple integrations (future)

Service traits live in `osg-core`. Adapters live in `osg-server` or `osg-desktop`.

## Standards

### Error handling

`thiserror` everywhere. No `anyhow`. Every error is a typed enum with matchable variants.

```rust
// Each module defines its own error enum
#[derive(Debug, thiserror::Error)]
pub enum PwError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("node {0} not found")]
    NodeNotFound(u32),
}

// osg-core re-exports a top-level error wrapping module errors
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error(transparent)]
    Pw(#[from] PwError),
    #[error(transparent)]
    Config(#[from] ConfigError),
}
```

### Logging levels

| Level | When | Target: <10 lines/session for info |
|-------|------|------------------------------------|
| `error!` | Unrecoverable, operator must act | PW connection lost, config corrupt, thread panic |
| `warn!` | Unexpected but self-recovering | Node disappeared mid-op, correction loop retrying |
| `info!` | Process lifecycle boundaries only | Server start/stop, PW connect/disconnect, version on startup |
| `debug!` | Every user-initiated mutation and result | Created sink, set volume, link created |
| `trace!` | Hot-path internals | Every PW graph event, POD serde, debounce ticks, reconcile diffs |

### TDD

- All tests in `tests/` directory. Zero `#[cfg(test)]` in source files.
- `tests/unit/` — pure logic (POD serialization, port mapping, identifier matching)
- `tests/integration/` — multi-module (graph reconciliation, reducer, config round-trip)
- `tests/e2e/` — requires running PipeWire daemon
- Red → Green → Refactor. Write the failing test first.

### DDD

- **MixerSession** is the only aggregate that mutates from user commands.
- **AudioGraph** is the only aggregate that mutates from PipeWire events.
- **ReconciliationService** is stateless — reads both, emits corrections.
- **Handlers are infrastructure** — translate events to external calls. No domain logic.
- Domain events flow one direction per pipe. No cycles.

### SOLID

- **S** — One handler, one command category, one concern. WebSocket broadcast is a separate handler.
- **O** — New command category = new handler struct implementing `EventHandler<E>` trait. Existing handlers untouched.
- **L** — Any `EventHandler<E>` impl is interchangeable. Mock handlers work identically in tests.
- **I** — Core API split into focused traits: `SinkManager`, `Router`, `VolumeControl`, `GraphObserver`. Consumers depend only on what they use.
- **D** — Handlers depend on traits, not concrete PipeWire types. Core is testable without a running PW daemon.

Frontend follows the same principles:
- **S** — One component, one concern. Matrix grid doesn't know about effects panel.
- **O** — New panel = new component subscribing to relevant events. Existing components untouched.
- **L** — Components depend on store interfaces (signals), not transport.
- **I** — Each store exposes only what its consumers need.
- **D** — Components read from reactive signals, not from WebSocket directly.

### Immutability

All state is immutable by default. Every mutation produces a new snapshot published via `watch` channel. No shared mutable references. In-place mutation only when explicitly justified with a comment explaining why.

### Naming

**Rust**: `snake_case` (standard).
**Wire format (JSON)**: `camelCase` via `#[serde(rename_all = "camelCase")]`.
**Frontend (TypeScript)**: `camelCase` — matches wire format, zero mapping.

### Domain glossary

| Term | Definition | Rejected synonyms |
|------|-----------|-------------------|
| **Channel** | User-created virtual audio bus (PW null-audio-sink) | GroupNode, Endpoint, VirtualSink |
| **Mix** | Output destination (headphones, stream, VOD) | Output, Bus, Destination |
| **Node** | PipeWire node in the graph (low-level) | Stream, SinkInput |
| **Route** | Connection between a channel and a mix | Link, Connection, Wire |
| **App** | Running application emitting audio | Application, Client, Stream |
| **CellVolume** | Per-route volume (L/R stereo) | RouteVolume, FaderLevel |
| **Preset** | Saved routing configuration | Scene, Snapshot, Profile |

### Frontend stack

- **SolidJS** — fine-grained reactivity, signals match `watch` channel pattern
- **Tailwind v4** — CSS-first config, utility classes
- **Style Dictionary v5 + DTCG JSON** — design tokens source of truth in `web/tokens/tokens.json`, generates CSS custom properties for any future frontend
- **Storybook 10** — component documentation and visual spec for future desktop reimplementation

### Dependencies

Use semver ranges. Encourage latest stable versions. Update frequently.

```toml
pipewire = "0.9"     # not "=0.9.2"
tokio = "1.50"       # not "=1.50.0"
```

### Git

Conventional commits: `type(scope): description` — `feat`, `fix`, `refactor`, `test`, `docs`, `chore`.

Recommended branch naming: `feat/short-description`, `fix/short-description`. Not enforced.

PR template (recommended, not enforced):
```
## What
## Why
## Test plan
```

### Effects architecture

Channels are pure signal (volume only). Effects belong on mixes — each mix processes each channel independently. Current per-channel effects in POC are convenience only. Target: per-mix effects via PipeWire filter-chain. Input pre-processing (mic gate, de-essing) = separate inputs page, not mixer matrix.

### PipeWire integration

OSG targets PipeWire exclusively (ADR-002). JACK apps work transparently via `pipewire-jack`. No `libjack` or `libpulse` dependencies. Integration via `pipewire-rs` 0.9 (adapted from Sonusmix, MPL-2.0).

Key patterns:
- Virtual sinks via `support.null-audio-sink` factory with ULID naming (`osg.group.{ulid}`)
- Stream routing via PipeWire metadata API (`target.object`)
- Per-channel L/R volume via SPA POD `channelVolumes` array
- Graph events via registry listeners, debounced at 16ms
- Correction loop: desired state vs PipeWire reality → corrective commands
- Dedicated PipeWire thread (blocking mainloop) + adapter thread (prevents async/PW deadlocks)
