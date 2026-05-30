## Task: fdemon-daemon cleanups — DeviceCapabilities export + log demotion

**Objective**: Apply the two daemon-layer minor findings from the Phase 5 review:
re-export `DeviceCapabilities` through the crate's public API (m4), and demote the
full-stdout `debug!` to reduce information density in logs (m5).

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 0.5–1h

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/lib.rs`
- `crates/fdemon-daemon/src/devices.rs`

**Files Read (Dependencies):**
- None.

### Details

**1. (m4 — MINOR) Re-export `DeviceCapabilities` from `lib.rs`.**
`DeviceCapabilities` is declared `pub` in `devices.rs` but omitted from the
`pub use devices::{...}` block in `crates/fdemon-daemon/src/lib.rs`, so it is only reachable
via the full module path while `Device` (its container) is re-exported at the crate root.
Add `DeviceCapabilities` to the same `pub use devices::{...}` list as `Device`, and update
the adjacent module doc comment (the one that mentions `Device`) to mention it. This keeps
the public surface consistent for when a future phase consumes the field.

**2. (m5 — MINOR) Demote the full-stdout `debug!`.**
`devices.rs:~238` logs the entire raw `flutter devices --machine` stdout at `debug!`. The
new `capabilities` data increases what is captured. Keep a concise `debug!` summary (e.g.,
byte length and/or parsed device count) and move the full payload to `trace!`:

```rust
debug!("flutter devices: {} bytes of stdout", stdout.len());
trace!("flutter devices stdout: {}", stdout);
```

Ensure `trace` is imported (via the project's `tracing` prelude/macros) — match how other
modules in the crate import tracing macros. Do not change any parsing logic or error paths.

### Acceptance Criteria

1. `fdemon_daemon::DeviceCapabilities` is reachable from the crate root (added to `pub use devices::{...}`), and the module doc comment references it alongside `Device`.
2. The full subprocess stdout is logged at `trace!`; a short summary remains at `debug!`.
3. No change to device parsing, `is_supported`/`capabilities` deserialization, or error handling.
4. `cargo test -p fdemon-daemon`, `cargo fmt`, `cargo clippy -p fdemon-daemon -- -D warnings` pass.

### Testing

- Existing `devices.rs` parsing tests must still pass unchanged.
- No new tests required (log level and re-export are not behavior the suite asserts on); a quick `cargo doc -p fdemon-daemon` sanity check that the re-export resolves is sufficient.

### Notes

- Pure non-functional cleanup; no behavior change for callers.
- Do not add `DeviceCapabilities` to any new serialized/persisted type — it is parse-and-store only this phase (see TASKS.md deferred note n1).
