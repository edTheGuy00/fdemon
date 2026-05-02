## Task: Harden `terminal.rs` — relax atomic ordering, idempotent panic hook, document invariants

**Objective**: Apply three small hardening fixes to `crates/fdemon-tui/src/terminal.rs`: (1) downgrade `Ordering::SeqCst` on `MOUSE_CAPTURE_ON` to the minimal ordering that preserves correctness, (2) make `install_panic_hook()` idempotent against double-install, (3) add inline comments documenting two non-obvious invariants (DECSET 1003 trade-off and panic-hook cleanup ordering). Bundled into one task because all three changes touch the same file and are individually trivial.

**Depends on**: None

**Estimated Time**: 1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/terminal.rs`:
  - Replace `Ordering::SeqCst` with appropriate `Release` / `Acquire` / `AcqRel` / `Relaxed` orderings on `MOUSE_CAPTURE_ON`.
  - Add a `static HOOK_INSTALLED: AtomicBool` and early-return from `install_panic_hook()` if it has already run.
  - Add inline comments explaining the DECSET 1003 trade-off and the panic-hook cleanup ordering invariant.

**Files Read (Dependencies):**
- None.

### Details

#### Sub-task A — Relax atomic ordering on `MOUSE_CAPTURE_ON`

Currently every access uses `Ordering::SeqCst` (lines 58, 72, 91, 103, 114, 118, 128). `SeqCst` provides a global total order across all atomics, which is overkill for a single flag. The actual happens-before relationship needed is: the `EnableMouseCapture` `execute!` writes the terminal sequences before the flag is set; the flag must be visible-as-true to a later thread before that thread can decide whether to call `DisableMouseCapture`.

Recommended ordering:

| Site | Current | Recommended | Rationale |
|------|---------|-------------|-----------|
| `enable_mouse_capture` store after success (line ~58) | `SeqCst` | `Release` | Pairs with `Acquire` swap in `disable`; ensures the `execute!` writes happen-before the store |
| `disable_mouse_capture` swap (line ~72) | `SeqCst` | `Acquire` | Reads the flag and pairs with the `Release` store |
| Tests that reset the flag (lines ~91, ~103, ~114) | `SeqCst` | `Relaxed` | No synchronization needed; the test's serial gate provides ordering |
| Tests that observe the flag (lines ~118, ~128) | `SeqCst` | `Acquire` | Pairs with any `Release` store from the production code being tested |

If you prefer `AcqRel` on the swap (it both acquires the previous value and releases the new value), that is also correct and arguably clearer — choose whichever feels most natural; both produce identical behavior here.

#### Sub-task B — Idempotent `install_panic_hook()`

Each call to `install_panic_hook()` calls `std::panic::take_hook()` and then `std::panic::set_hook()` to wrap the previous hook with mouse-disable + ratatui-restore logic. If both `run_with_project` and `run_with_project_and_dap` were ever invoked in the same process (today they are mutually exclusive entry points, but this is an implicit contract), the wrap chain would contain two mouse-disables.

Add an idempotency guard mirroring the existing `MOUSE_CAPTURE_ON` pattern in the same file:

```rust
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

pub fn install_panic_hook() {
    // Idempotency guard: each entry-point runner calls this; multiple calls
    // in one process would chain duplicate mouse-disable / ratatui-restore
    // closures, so we install at most once per process.
    if HOOK_INSTALLED.swap(true, Ordering::AcqRel) {
        return;
    }

    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        disable_mouse_capture();
        ratatui::restore();
        original(panic_info);
    }));
}
```

Add a unit test (preferably under `#[serial]` like the existing terminal tests) that calls `install_panic_hook()` twice and verifies the second call is a no-op. You can verify either by observing `HOOK_INSTALLED` directly or by checking that `take_hook()` after two installs returns a hook chain of the expected depth — whichever is easier given the existing test scaffolding.

#### Sub-task C — Inline comments documenting two invariants

**Comment 1: DECSET 1003 trade-off.** Near the `EnableMouseCapture` call (around line ~55), add:

```rust
// crossterm::EnableMouseCapture emits DECSET 1000/1002/1003/1015/1006.
// We include 1003 (any-motion) even though `Moved` events are dropped at the
// event.rs boundary. This trade-off keeps capture-mode setup symmetric with
// crossterm's defaults; consumers that need to minimize per-frame parser cost
// should switch to a tighter mode set (e.g. only 1002 button-event motion)
// when `Moved` events become useful in a future phase.
execute!(stdout(), EnableMouseCapture).map_err(|e| { ... })?;
```

**Comment 2: Panic-hook cleanup ordering.** Inside `install_panic_hook()` (around lines 36–37), add:

```rust
// disable_mouse_capture() must run before ratatui::restore() so the DECRST
// sequences are emitted while the alt screen is still active. In practice,
// DECSET/DECRST mouse modes are connection-global (not alt-screen-scoped) so
// the ordering doesn't matter for cleanup correctness today — but this is a
// load-bearing assumption about ratatui's restore() implementation. Keep the
// disable-then-restore order to avoid coupling to that assumption changing.
disable_mouse_capture();
ratatui::restore();
```

Adjust wording to your taste; the goal is that a future maintainer touching either function understands why these orderings matter.

### Acceptance Criteria

1. `MOUSE_CAPTURE_ON` accesses use `Release` (or `AcqRel`) on stores/swaps and `Acquire` on loads. Test-only resets may use `Relaxed`. Zero `Ordering::SeqCst` references remain on this static.
2. `install_panic_hook()` is idempotent: calling it N times wraps the original hook exactly once. A new test verifies this (under `#[serial]`).
3. Inline comments explain the DECSET 1003 mode-set choice and the panic-hook cleanup ordering invariant. The comments name the actual modes (1000/1002/1003/1015/1006) and reference `event.rs`'s drop-on-`Moved` behavior.
4. All existing terminal tests still pass, plus the new idempotency test.
5. `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

```bash
cargo test -p fdemon-tui terminal
cargo clippy --workspace --all-targets -- -D warnings
```

For the idempotency test, follow the pattern of the existing `test_disable_without_enable_is_noop` etc. in `terminal.rs` (likely uses `#[serial_test::serial]` and resets `MOUSE_CAPTURE_ON`).

### Notes

- These three sub-tasks are bundled because they all touch one file and each is individually too small to merit a separate task. The orchestrator's worktree-merge cost would dominate the implementation cost otherwise.
- Do not refactor unrelated code in `terminal.rs` while you are here — keep the diff focused on these three concerns so the review is easy.
- The relaxed ordering change is unlikely to surface a behavior difference on x86 (which has strong native ordering), but matters on aarch64. Run the test suite on whichever target you have available; CI on the macOS + Linux + Windows matrix will exercise both x86 and ARM as appropriate.
