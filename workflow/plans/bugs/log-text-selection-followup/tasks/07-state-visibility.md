## Task: AppState `pending_runner_actions` visibility hygiene

**Objective:** Address the architecture reviewer's concern about `AppState::pending_runner_actions` being `pub`. The only legitimate accessor is `Engine::drain_runner_actions()`. Tighten visibility OR fix the misleading field comment to match reality.

**Depends on:** None

**Agent:** implementor

**Estimated time:** 30 minutes

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs`: change visibility of `pending_runner_actions` and update its doc-comment.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/engine.rs`: confirm `drain_runner_actions()` is the sole legitimate access path.
- `crates/fdemon-app/src/process.rs`: confirm `process.rs:78-82` is the only push site.

### Details

Current code at `crates/fdemon-app/src/state.rs:1218` (approximately — search for `pending_runner_actions`):

```rust
/// Runner-side actions queued by `process.rs` for synchronous execution by the
/// TUI runner. `pub` so the runner (`fdemon-tui`) can drain it directly without
/// a dedicated accessor, following the same pattern as `toasts`.
pub pending_runner_actions: Vec<crate::handler::UpdateAction>,
```

The comment is wrong: the runner does NOT drain it directly. It calls `engine.drain_runner_actions()` (defined in `engine.rs:391`), which calls `std::mem::take(&mut self.state.pending_runner_actions)`. The dedicated accessor exists.

**Two fix options:**

**Option A (preferred):** Drop the `pub` keyword. The push site (`process.rs:78-82`) is in the same crate as the field, so `pub(crate)` works there. The drain site (`engine.rs:391`) is also in the same crate. The runner accesses only via `engine.drain_runner_actions()`, which is the public API surface. So `pending_runner_actions` does not need to be `pub` — only `pub(crate)` (the default for a `pub`-less field on a `pub` struct).

```rust
/// Runner-side actions queued by `process.rs` for synchronous execution by the
/// TUI runner. Drained via `Engine::drain_runner_actions()`. NOT directly
/// accessible from outside this crate — direct mutation would bypass the
/// `process.rs` routing gate.
pub(crate) pending_runner_actions: Vec<crate::handler::UpdateAction>,
```

Verify: this is in a `pub struct AppState`. A field without explicit visibility is private (module-only), which is even stricter. If `pub(crate)` works for both push and drain sites (it should, both are in `fdemon-app`), use that.

**Option B (fallback):** If `pub(crate)` breaks compilation because some test or doc-test reaches in directly, keep `pub` but rewrite the comment to NOT lie:

```rust
/// Runner-side actions queued by `process.rs` for synchronous execution by the
/// TUI runner. The legitimate access path is `Engine::drain_runner_actions()` —
/// do not drain or push to this `Vec` directly from outside the crate. The
/// `pub` visibility exists only because [reason]; future cleanup may narrow it.
pub pending_runner_actions: Vec<crate::handler::UpdateAction>,
```

Try Option A first. If it works (most likely), use it. If it doesn't, document why in the field comment per Option B.

### Acceptance Criteria

1. Either:
   - `pending_runner_actions` is no longer `pub` (it's `pub(crate)` or unannotated), AND `cargo build --workspace` passes; OR
   - `pending_runner_actions` retains `pub` but the field comment accurately describes the access contract (no false claim that the runner drains it directly).
2. The field's comment no longer says "`pub` so the runner can drain it directly" if it doesn't actually do that.
3. `cargo build --workspace` and `cargo test --workspace` pass.

### Testing

`cargo build --workspace` and `cargo test --workspace`. No new tests.

### Notes

- The architecture reviewer's main concern was that `pub` on `AppState` allows a future contributor (or the headless runner) to drain or push directly, bypassing the `process.rs` routing gate. Tightening to `pub(crate)` or fixing the doc-comment both address the concern; tightening is preferred.
- Do NOT change the field name or type. Only visibility + comment.
