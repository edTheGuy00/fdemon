# Task 02 — Clipboard service (trait + arboard impl + memory mock)

**Agent:** implementor
**Wave:** 1
**Depends on:** — (parallel with 01, 03)
**Files written:**
- `crates/fdemon-app/src/services/clipboard.rs` *(new)*
- `crates/fdemon-app/src/services/mod.rs` *(add the new module + re-exports)*
- `crates/fdemon-app/Cargo.toml` *(add `arboard` dep)*

---

## Goal

Introduce a `Clipboard` trait with two implementations: a real one backed by [`arboard`](https://docs.rs/arboard/) for the runner, and a `MemoryClipboard` for tests. Exposes a single method:

```text
fn write_text(&mut self, text: &str) -> Result<()>
```

This service is consumed by the right-click-copy handler in Task 04 and the update-handler arm in Task 06.

## Why a service abstraction

- Tests in the workspace run headless (no X11/Wayland display); `arboard` fails to initialize on Linux CI without a display. A `MemoryClipboard` impl lets handler tests assert "the right text was written" deterministically.
- Mirrors the existing `crates/fdemon-app/src/services/` pattern (`flutter_controller.rs`, `log_service.rs`, `state_service.rs`).

## Implementation

1. Add to `crates/fdemon-app/Cargo.toml`:

   ```toml
   arboard = { version = "3", default-features = false }
   ```

   Confirm the version against `cargo search arboard` at implementation time; pick the current major-version line.

2. Create `crates/fdemon-app/src/services/clipboard.rs`:

   ```text
   pub trait Clipboard: Send {
       fn write_text(&mut self, text: &str) -> fdemon_core::Result<()>;
   }

   pub struct SystemClipboard { inner: arboard::Clipboard }
   impl SystemClipboard {
       pub fn new() -> fdemon_core::Result<Self> { ... }   // wraps arboard::Clipboard::new()
   }
   impl Clipboard for SystemClipboard { ... }              // maps arboard::Error -> fdemon_core::Error::terminal

   #[derive(Default)]
   pub struct MemoryClipboard { pub writes: Vec<String> }
   impl Clipboard for MemoryClipboard {
       fn write_text(&mut self, text: &str) -> fdemon_core::Result<()> {
           self.writes.push(text.to_string());
           Ok(())
       }
   }
   ```

3. In `crates/fdemon-app/src/services/mod.rs`, add `pub mod clipboard;` and re-export `Clipboard`, `SystemClipboard`, `MemoryClipboard`.

4. The `Clipboard` instance is owned by the runner (the only side-effect-bearing layer) and threaded into the handler at message-dispatch time. **Do NOT** add a clipboard field to `AppState` — that would mix side-effect handles into the model. Task 06 will route the trait reference through the existing handler signature (it may need to extend the signature; see Task 06).

## Tests

- `test_memory_clipboard_records_writes` — write twice, assert both entries in `writes`.
- `test_memory_clipboard_returns_ok` — write returns `Ok`.
- `SystemClipboard` is **not** unit-tested (requires a real clipboard); manual verification only.

## Acceptance Criteria

- [ ] `cargo build -p fdemon-app` succeeds with `arboard` as a new dep.
- [ ] Both impls compile under `cargo clippy --workspace -- -D warnings`.
- [ ] Two new unit tests pass.
- [ ] `services/mod.rs` re-exports the new types so consumers can `use fdemon_app::services::Clipboard`.
- [ ] No `AppState` change in this task (Task 03 owns state changes).

## Notes for Reviewer

- `arboard` brings in a transitive X11/Wayland dep chain on Linux. Keep `default-features = false` and pick the minimum features required for `write_text`.
- Errors are mapped to `fdemon_core::Error::terminal(...)`. The `terminal` variant fits because clipboard failures originate from the OS-level terminal context the user is running in.
- `Send` bound on the trait is required because the runner may construct it on one thread and use it on the TEA dispatch thread.

---

## Completion Summary

**Status:** Done
**Branch:** plan/log-text-selection-fix

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/Cargo.toml` | Added `arboard = { version = "3", default-features = false }` |
| `crates/fdemon-app/src/services/clipboard.rs` | New file: `Clipboard` trait, `SystemClipboard` (arboard-backed), `MemoryClipboard` (in-memory), 2 unit tests, 1 doc test |
| `crates/fdemon-app/src/services/mod.rs` | Added `pub mod clipboard;` and re-exported `Clipboard`, `SystemClipboard`, `MemoryClipboard` |

### Notable Decisions/Tradeoffs

1. **`arboard` not added to workspace deps**: The dependency is only needed in `fdemon-app` and adding it to `[workspace.dependencies]` is not required when only one crate uses it. Kept it local to the crate's `Cargo.toml` for clarity.
2. **Doc test in `MemoryClipboard`**: Added a working doc example that doubles as a third test, costing nothing in CI runtime.
3. **`pub mod clipboard` (not `mod`)**: The module is made public so `fdemon_app::services::clipboard::MemoryClipboard` is reachable directly in addition to the re-exported paths. This mirrors no existing precedent but is the safer choice for downstream test authors.

### Testing Performed

- `cargo build -p fdemon-app` - Passed (arboard 3.6.1 resolved and compiled)
- `cargo test -p fdemon-app test_memory_clipboard` - Passed (2 tests)
- `cargo test -p fdemon-app --doc` - Passed (doc test for `MemoryClipboard` also runs)
- `cargo test --workspace` - Passed (all 3,335+ tests across workspace)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no warnings)
- `cargo fmt --all -- --check` - Passed

### Risks/Limitations

1. **Linux CI**: `arboard` with `default-features = false` still pulls in X11/Wayland transitive deps on Linux but does not require a running display at compile time. `SystemClipboard::new()` will fail at runtime on headless Linux, which is why the `MemoryClipboard` mock exists for tests.
2. **No `AppState` changes**: As required by the task. Task 03 owns state additions, Task 06 owns handler wiring.
