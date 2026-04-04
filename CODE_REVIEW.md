# Code Review Report — Open Sound Grid

**Date:** 2026-04-01  
**Scope:** `crates/osg-core`, `crates/osg-server`, `web/src`  
**Focus:** Architecture design, Concurrency safety, Audio pipeline, Frontend consistency  
**Approach:** Broad then deep — anti-pattern scan followed by targeted analysis

---

## Executive Summary

| Metric | Count |
|--------|-------|
| **Total Issues** | 13 |
| **P0 Critical** | 3 |
| **P1 High** | 4 |
| **P2 Medium** | 3 |
| **P3 Low** | 3 |
| **Architecture Threats** | 3 |
| **Test Coverage Gaps** | 5 |

**Blast Radius:** The P0 issues affect the real-time audio path and can cause:
- Audio glitches (bypass race condition)
- Memory leaks during long sessions
- Panics in PW callbacks (crash)

---

## Critical Issues (P0)

### P0-1: Memory Ordering Race in Filter Bypass

**File:** `crates/osg-core/src/pw/filter.rs:196, 209`

**Code:**
```rust
pub fn set_bypassed(&self, bypassed: bool) {
    self.bypassed.store(bypassed, Ordering::Relaxed);
}

pub fn set_eq(&self, config: &EqConfig) {
    self.eq.store(Arc::new(CompiledEq::from_config(config)));
    if !config.bands.is_empty() {
        self.bypassed.store(false, Ordering::Relaxed);  // Line 209
    }
}
```

**Problem:** Using `Ordering::Relaxed` for the bypass flag is insufficient. When the RT audio thread reads `bypassed` and it's false, it immediately reads the EQ params via `load_eq()`. Without proper memory ordering, the RT thread could see:
1. `bypassed = false` (new value)
2. EQ params still pointing to old/empty coefficients

This causes audio glitches or incorrect DSP processing.

**Fix:**
```rust
pub fn set_bypassed(&self, bypassed: bool) {
    self.bypassed.store(bypassed, Ordering::Release);
}

pub fn is_bypassed(&self) -> bool {
    self.bypassed.load(Ordering::Acquire)
}
```

**Alternatively, use `Ordering::SeqCst` for simplicity.**

---

### P0-2: Metadata Listener Memory Leak

**File:** `crates/osg-core/src/pw/mainloop.rs:448-450`

**Code:**
```rust
// Store the proxy so we can call set_property later. Leak the listener.
*metadata_out.borrow_mut() = Some(metadata);
std::mem::forget(listener);
```

**Problem:** The listener is intentionally leaked to keep it alive for the PW mainloop lifetime. However:
1. If `init_mainloop` is called multiple times (unlikely but possible in tests), listeners accumulate
2. No shutdown hook to clean up the metadata proxy

**Fix:** 
1. Store the listener in `Master` struct instead of leaking
2. Add proper cleanup in the `Exit` message handler
3. Or document that this is intentional for process-lifetime resources

---

### P0-3: RefCell Panic Risk in PW Callbacks

**File:** `crates/osg-core/src/pw/mainloop.rs:352-358`

**Code:**
```rust
.param({
    move |_, type_, _, _, pod| {
        let mut store_borrow = store.borrow_mut();  // Can panic!
        store_borrow.update_node_param(type_, id, pod);
        let _ = sender.send(ToPipewireMessage::Update);
    }
})
```

**Problem:** `borrow_mut()` inside PW callbacks can panic if:
1. Another `borrow_mut()` is active (re-entrant callback)
2. Multiple threads access the same `RefCell` (not thread-safe)

The entire architecture uses `Rc<RefCell<Store>>` which is not safe for re-entrancy. PW callbacks can nest when:
- Node param update triggers registry event
- Graph update triggers another callback

**Fix:**
```rust
.param({
    move |_, type_, _, _, pod| {
        if let Ok(mut store_borrow) = store.try_borrow_mut() {
            store_borrow.update_node_param(type_, id, pod);
            let _ = sender.send(ToPipewireMessage::Update);
        } else {
            // Log and skip this update - re-entrant call
            tracing::warn!("Re-entrant update_node_param skipped for node {}", id);
        }
    }
})
```

---

## High Issues (P1)

### P1-1: Stereo Volume POD Array Validation

**File:** `crates/osg-core/src/pw/pod.rs:180-186`

**Code:**
```rust
pub fn build_node_volume_pod(channel_volumes: Vec<f32>) -> (ParamType, PodBytes) {
    let pod = Value::Object(object! {
        SpaTypes::ObjectParamProps,
        ParamType::Props,
        Property::new(SPA_PROP_channelVolumes, Value::ValueArray(ValueArray::Float(channel_volumes))),
    }).serialize();
    (ParamType::Props, pod)
}
```

**Problem:** Memory indicates that `cv.set(2, vol)` is required — single-element arrays silently fail to set volume on stereo nodes. No validation that `channel_volumes.len() == 2`.

**Fix:**
```rust
pub fn build_node_volume_pod(channel_volumes: Vec<f32>) -> (ParamType, PodBytes) {
    debug_assert!(channel_volumes.len() == 2, "Stereo requires exactly 2 channel volumes");
    if channel_volumes.len() == 1 {
        // Single channel: duplicate for stereo
        let stereo = vec![channel_volumes[0], channel_volumes[0]];
        return build_node_volume_pod(stereo);
    }
    // ... rest of implementation
}
```

---

### P1-2: Correction Loop Risk

**File:** `crates/osg-core/src/routing/reconcile.rs:48-64`

**Code:**
```rust
pub fn diff(&mut self, graph: &AudioGraph, settings: &ReconcileSettings) -> Vec<ToPipewireMessage> {
    let endpoint_nodes = self.diff_nodes(graph, settings);
    let mut messages = self.auto_create_app_channels(graph);
    self.ensure_default_links();
    messages.extend(self.diff_channels(&endpoint_nodes, graph));
    // ... more diff_* calls
}
```

**Problem:** No tracking of reconciliation depth or generation. If `diff_properties` emits a `NodeVolume` command that triggers:
1. PW updates the node
2. PW emits info event
3. `update_node_info` is called
4. `ToPipewireMessage::Update` is sent
5. Graph is dumped
6. Another `diff()` is called

This can oscillate indefinitely without detection.

**Fix:** Add reconciliation tracking:
```rust
pub fn diff(&mut self, graph: &AudioGraph, settings: &ReconcileSettings) -> Vec<ToPipewireMessage> {
    self.reconcile_depth += 1;
    if self.reconcile_depth > MAX_RECONCILE_DEPTH {
        tracing::warn!("Reconciliation depth exceeded, skipping");
        self.reconcile_depth -= 1;
        return vec![];
    }
    // ... existing code ...
    self.reconcile_depth -= 1;
    messages
}
```

---

### P1-3: Missing Volume Bounds Validation

**File:** `crates/osg-core/src/graph/types.rs:315, 356`

**Code:**
```rust
pub volume: f32,
pub volume_left: f32,
pub volume_right: f32,
```

**Problem:** No validation that volumes are in valid range [0.0, 1.5] (or [0.0, 1.0] for unity). UI could send out-of-range values leading to:
- Distortion (volume > 1.0)
- Silent channels (negative values interpreted as NaN or large)

**Fix:** Add validation in setters or use bounded types:
```rust
impl Endpoint {
    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.5);
    }
}
```

---

### P1-4: Unsafe Code Without Safety Comments

**File:** `crates/osg-core/src/pw/filter.rs:10`

**Code:**
```rust
#![allow(unsafe_op_in_unsafe_fn)]
```

**Problem:** Per ADR-005 and Rust best practices, every `unsafe` block should have a `// SAFETY:` comment explaining why the operation is sound. This module has extensive unsafe FFI but no safety documentation.

**Fix:** Add `// SAFETY:` comments to each unsafe block:
```rust
// SAFETY: pw_core.as_raw_ptr() returns a valid PW core pointer that remains
// valid for the lifetime of OsgFilter because it's created on the PW thread
// and destroyed before the thread exits.
let filter_result = unsafe {
    super::filter::OsgFilter::new(pw_core.as_raw_ptr(), &filter_name, &format!("EQ: {name}"))
};
```

---

## Medium Issues (P2)

### P2-1: Clippy Allowances Without Justification

**File:** `crates/osg-core/src/pw/mainloop.rs:453-457`

**Code:**
```rust
#[allow(clippy::type_complexity, clippy::too_many_lines, clippy::cognitive_complexity)]
pub(super) fn init_mainloop(...)
```

**Problem:** The function is 357 lines, exceeding the 30-line guideline in `CLAUDE.md`. The `too_many_lines` lint is suppressed rather than fixing the underlying complexity.

**Recommendation:** Refactor `init_mainloop` into smaller functions:
- `init_pw_core()`
- `init_registry()`
- `init_message_handler()`
- This also improves testability

---

### P2-2: Expect Without Context

**File:** `crates/osg-core/src/routing/reconcile.rs:143, 154, 164`

**Code:**
```rust
let channel_kind = self.channels.get(&id).expect("channel must exist").kind;
let channel = self.channels.get_mut(&id).expect("channel must exist");
```

**Problem:** Using `expect` is better than `unwrap`, but the panic message doesn't include the offending ID, making debugging harder.

**Fix:**
```rust
let channel = self.channels.get(&id)
    .expect(&format!("channel {} must exist", id.inner()));
```

---

### P2-3: Hardcoded Sample Rate

**File:** `crates/osg-core/src/pw/filter.rs:26`

**Code:**
```rust
const SAMPLE_RATE: f32 = 48000.0;
```

**Problem:** PipeWire can run at 44100Hz, 96000Hz, or other sample rates. The EQ biquad coefficients are computed for 48kHz, leading to frequency skew at other rates.

**Recommendation:** Query the actual sample rate from PipeWire or make it configurable per filter.

---

## Low Issues (P3)

### P3-1: Unimplemented PersistentNode Resolution

**File:** `crates/osg-core/src/routing/reconcile.rs:643-646`

```rust
EndpointDescriptor::PersistentNode(_id, _kind) => {
    // TODO: Implement persistent node matching.
    None
}
```

Persistent nodes (matched by name across PW restarts) are never resolved. This feature is documented but not implemented.

---

### P3-2: Unimplemented Device Resolution

**File:** `crates/osg-core/src/routing/reconcile.rs:699-702`

```rust
EndpointDescriptor::Device(_id, _kind) => {
    // TODO: Implement device resolution.
    None
}
```

Device endpoints are never resolved. This limits functionality for hardware device routing.

---

### P3-3: Missing Debug Implementation

**File:** `crates/osg-core/src/routing/reconcile.rs:28`

```rust
#[allow(missing_debug_implementations)]
pub struct ReconciliationService;
```

This is a stateless service, so `Debug` isn't critical, but should be documented why it's allowed.

---

## Architecture Threats

### A1: DDD Boundary Violation — Handler Contains Logic

**File:** `crates/osg-core/src/pw/mainloop.rs:762-798`

The message handler in `init_mainloop` contains business logic:
- Effects parameter conversion (ms → seconds)
- Peak storage by ULID lookup
- Cell node name formatting

Per DDD, handlers should only translate domain events to external API calls. The effects conversion should be in a domain service.

**Recommendation:** Extract effects conversion to a domain helper function called before the PW message is created.

---

### A2: Multiple Paths Mutate Store via RefCell

**File:** `crates/osg-core/src/pw/mainloop.rs`

The `Store` (via `Rc<RefCell<Store>>`) is mutated from:
1. `registry_listener()` — node/device add/remove
2. `init_node_listeners()` — node info/param updates  
3. `init_device_listeners()` — device route updates
4. Message handler — explicit mutations

All use `borrow_mut()` which panics on collision. Re-entrant PW callbacks are possible.

**Recommendation:** Consider using `Mutex` instead of `RefCell` for thread safety, or use `try_borrow_mut()` with graceful degradation.

---

### A3: Tight Coupling Between MixerSession and AudioGraph

**File:** `crates/osg-core/src/routing/reconcile.rs:34-40`

```rust
pub fn reconcile(
    state: &mut MixerSession,
    graph: &AudioGraph,
    settings: &ReconcileSettings,
) -> Vec<ToPipewireMessage> {
    state.diff(graph, settings)
}
```

Per ADR-003, `MixerSession` (write model) should not read `AudioGraph` (read model). However, `diff()` takes `graph` as input. This is intentional for stateless reconciliation, but creates coupling.

**Status:** Acceptable — this is the documented pattern for reconciliation services that need both sides.

---

## Test Coverage Gaps

| Area | Gap Description | Risk Level |
|------|-----------------|------------|
| Reconciliation | No tests for correction loop detection | High |
| Concurrency | No tests for re-entrant PW callback | High |
| Filter Bypass | No tests for RT/main race on bypass toggle | High |
| Volume Bounds | No tests for [0.0, 1.5] validation | Medium |
| WebSocket | No tests for reconnection handling | Medium |

**Existing Tests:**
```
tests/biquad_test.rs      — 174 lines (biquad math)
tests/eq_config_test.rs   — 159 lines (EQ config round-trip)
tests/filter_test.rs      — 153 lines (filter creation)
tests/port_mapping_test.rs — 103 lines (port mapping)
```

---

## Prioritized Fix List

| Rank | Issue | Severity | Files | Effort |
|------|-------|----------|-------|--------|
| 1 | Memory ordering race | P0 | `pw/filter.rs` | 30 min |
| 2 | RefCell panic risk | P0 | `pw/mainloop.rs` | 1 hr |
| 3 | Stereo POD validation | P1 | `pw/pod.rs` | 15 min |
| 4 | Volume bounds | P1 | `graph/types.rs` | 30 min |
| 5 | Correction loop depth | P1 | `routing/reconcile.rs` | 1 hr |
| 6 | Safety comments | P1 | `pw/filter.rs` | 1 hr |
| 7 | Metadata listener cleanup | P0 | `pw/mainloop.rs` | 45 min |
| 8 | Clippy suppression | P2 | `pw/mainloop.rs` | 2 hr (refactor) |
| 9 | Expect with ID | P2 | `routing/reconcile.rs` | 30 min |
| 10 | Sample rate | P2 | `pw/filter.rs` | 2 hr |

---

## Verification Commands

```bash
# Check for unwrap/expect in PW mainloop
grep -n "unwrap\|expect" crates/osg-core/src/pw/mainloop.rs

# Check for Ordering in atomics
grep -n "Ordering::" crates/osg-core/src/pw/filter.rs

# Check for volume bounds validation
grep -n "volume.*clamp\|volume.*validate" crates/osg-core/src/graph/types.rs

# Run tests
cargo test -p osg-core
```

---

## Recommendations

1. **Immediate:** Fix P0 issues before next release — they affect audio reliability
2. **Short-term:** Add `try_borrow_mut()` error handling to prevent panics
3. **Medium-term:** Refactor `init_mainloop` into smaller functions
4. **Long-term:** Add integration tests for reconciliation scenarios

---

*Generated by Claude Code Review — 2026-04-01*