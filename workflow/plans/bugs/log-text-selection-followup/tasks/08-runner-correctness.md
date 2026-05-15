## Task: Runner correctness — try_send fallback + NullClipboard adoption + exhaustive match

**Objective:** Three runner-side correctness fixes that all touch `crates/fdemon-tui/src/runner.rs` and must therefore land together:

1. **`try_send` fallback path:** If the message channel is full when sending `Message::MouseCaptureChanged`, write `state.mouse_capture_active = target` directly (and push a warn toast) instead of dropping the message and lying about state forever.
2. **`NullClipboard` adoption:** Substitute `NullClipboard` (added in task 01) at the three runner-fallback sites where `SystemClipboard::new()` fails. Add a startup `ToastLevel::Warn` toast so the user sees the degraded state at startup.
3. **Exhaustive match:** Replace the `_ => warn!(...)` catch-all in `handle_runner_actions` with explicit unhandled arms for every `UpdateAction` variant, so the compiler enforces awareness when a new runner-side variant is added.

**Depends on:** Task 01 (`NullClipboard`)

**Agent:** implementor

**Estimated time:** 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/runner.rs`: three changes outlined below.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/services/clipboard.rs`: `NullClipboard` (from task 01).
- `crates/fdemon-app/src/handler/mod.rs`: `UpdateAction` enum (for the exhaustive match).
- `crates/fdemon-app/src/state.rs`: `AppState::mouse_capture_active` and `push_toast` API.

### Details

#### 1. `try_send` fallback path

Current code at `crates/fdemon-tui/src/runner.rs:331-335` (approximately — search for `try_send` near `MouseCaptureChanged`):

```rust
if let Err(e) = engine.msg_sender().try_send(Message::MouseCaptureChanged { active: target }) {
    warn!("failed to enqueue MouseCaptureChanged follow-up: {e}");
}
```

The drop is a correctness bug: `state.mouse_capture_active` never updates, the badge lies indefinitely, and the next `Alt+m` press computes the wrong target via the now-stale state.

Replace with:

```rust
if let Err(e) = engine.msg_sender().try_send(Message::MouseCaptureChanged { active: target }) {
    // Channel is saturated. The MouseCaptureChanged handler would have set
    // state.mouse_capture_active = target and pushed a status toast. Apply
    // those side effects directly here so the model does not lie about the
    // terminal state. Direct state mutation from the runner is a deliberate
    // exception to the TEA "single update site" rule, justified because we are
    // reflecting an already-observed terminal state change that the message
    // would have applied if the channel had capacity.
    error!("MouseCaptureChanged channel full; applying state directly: {e}");
    engine.state_mut().mouse_capture_active = target;
    engine.state_mut().push_toast(
        ToastLevel::Warn,
        if target {
            "Mouse capture on (channel full; state applied directly)"
        } else {
            "Mouse capture off (channel full; state applied directly)"
        }.to_string(),
    );
}
```

(Verify exact `state_mut()` and `push_toast` accessors against the actual API.)

#### 2. `NullClipboard` adoption + startup toast

Current code at `crates/fdemon-tui/src/runner.rs:31-37` (and analogous blocks at lines 142-148, 213):

```rust
let clipboard: Box<dyn Clipboard> = match SystemClipboard::new() {
    Ok(c) => Box::new(c),
    Err(e) => {
        warn!("system clipboard unavailable: {e}");
        Box::new(MemoryClipboard::default())
    }
};
```

Replace with:

```rust
let (clipboard, clipboard_unavailable_reason): (Box<dyn Clipboard>, Option<String>) =
    match SystemClipboard::new() {
        Ok(c) => (Box::new(c), None),
        Err(e) => {
            let reason = format!("{e}");
            warn!("system clipboard unavailable: {reason}");
            (Box::new(NullClipboard), Some(reason))
        }
    };
```

Then, after the engine is constructed, push a one-shot startup toast if `clipboard_unavailable_reason.is_some()`:

```rust
if let Some(reason) = clipboard_unavailable_reason {
    engine.state_mut().push_toast(
        ToastLevel::Warn,
        format!("Clipboard unavailable; right-click copy is disabled ({reason})"),
    );
}
```

Apply the same change at all THREE fallback sites (`run_with_project`, `run_with_project_and_dap`, `run`).

The startup toast ensures the user sees the degraded state on launch — they don't have to right-click first to discover the clipboard is broken. Combined with the `NullClipboard::write_text` returning `Err`, the user gets a second `Clipboard write failed` toast on each subsequent right-click attempt.

#### 3. Exhaustive match in `handle_runner_actions`

Current code at `crates/fdemon-tui/src/runner.rs:323-360` (approximately):

```rust
match action {
    UpdateAction::SetMouseCapture(enabled) => { /* ... */ }
    UpdateAction::WriteClipboard { text } => { /* ... */ }
    _ => {
        warn!("unexpected runner action: {action:?}");
    }
}
```

Replace `_` with an explicit list of UNHANDLED variants. This forces the compiler to error if a new `UpdateAction` variant is added without an explicit decision about whether the runner should handle it. List every variant currently in `UpdateAction` (read `crates/fdemon-app/src/handler/mod.rs` to enumerate; there are ≈25 variants).

Pattern:

```rust
match action {
    // Runner-handled side effects:
    UpdateAction::SetMouseCapture(enabled) => { /* existing body */ }
    UpdateAction::WriteClipboard { text } => { /* existing body */ }

    // Variants that should NEVER reach the runner queue (they go through
    // process.rs::handle_action). If one does land here, it indicates a routing
    // bug in process.rs; warn but do not panic.
    UpdateAction::SpawnFlutterRun { .. }
    | UpdateAction::ReloadSession { .. }
    | UpdateAction::RestartSession { .. }
    // ... enumerate all remaining variants ...
    => {
        warn!("runner action queue received non-runner variant: {action:?}");
    }
}
```

If the enum is too long to make this practical, gate the exhaustiveness check via a compile-time helper:

```rust
// Compile-time hint: when adding a new UpdateAction variant, decide whether it
// belongs to the runner queue. Add it to the runner-handled arms above OR to the
// non-runner arms below.
```

Pick whichever approach the existing codebase patterns suggest. The architecture-reviewer flagged this as a maintenance risk; either explicit listing or a `#[non_exhaustive]` annotation + explicit match arms achieves the goal.

### Acceptance Criteria

1. **try_send fallback:** When the channel is saturated and `MouseCaptureChanged` cannot be enqueued, `state.mouse_capture_active` reflects `target` and a `ToastLevel::Warn` toast informs the user of the channel-full event.
2. **NullClipboard adoption:** All THREE runner-fallback sites use `NullClipboard` (not `MemoryClipboard`).
3. **Startup toast:** When `SystemClipboard::new()` fails, a `ToastLevel::Warn` toast fires before the user does any right-clicks, naming the unavailability and the disabled feature.
4. **Failure-toast on right-click:** Right-click against `NullClipboard` produces the existing `Clipboard write failed` toast (because `NullClipboard::write_text` returns `Err`).
5. **Exhaustive match:** The `_` catch-all in `handle_runner_actions` is gone or only catches truly future-proof additions; current `UpdateAction` variants are explicitly enumerated.
6. New unit tests:
   - `test_mouse_capture_changed_channel_full_applies_state_directly` — simulate a full channel and verify state + toast.
   - `test_runner_uses_null_clipboard_when_system_unavailable` (or equivalent) — verify the startup toast fires.
7. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` all pass.

### Testing

Add tests in `crates/fdemon-tui/src/runner.rs::tests` near the existing `test_set_mouse_capture_action_enqueues_followup_message`:

```rust
#[tokio::test]
async fn test_mouse_capture_changed_channel_full_applies_state_directly() {
    // Construct an Engine with the message channel artificially filled to capacity.
    // Verify that handle_runner_actions on a SetMouseCapture(true) action results
    // in state.mouse_capture_active = true even though try_send fails.
    // (Implementation depends on Engine's test API.)
}

#[tokio::test]
async fn test_null_clipboard_returns_err_and_runner_pushes_toast() {
    // Use NullClipboard; dispatch UpdateAction::WriteClipboard { text: "hi" };
    // verify a "Clipboard write failed" toast is pushed.
}
```

If filling the channel to capacity is impractical in a test, consider a different verification strategy (e.g., using a custom `Sender` mock).

### Notes

- This task is in **Wave 2** because it depends on Task 01's `NullClipboard`.
- The `try_send` fallback's direct state mutation is a deliberate TEA exception. Document the rationale clearly in the inline comment so future readers don't think it's a bug to "clean up".
- Do NOT switch to `blocking_send().await` — the codebase universally uses `try_send` for runner-side sends, and `run_loop` is synchronous (not an async fn).
- Do NOT modify `services/clipboard.rs` (Task 01's territory) or `handler/update.rs` (the handler-side "Copied: …" optimistic toast is left as-is per BUG.md "Further Considerations").

---

## Completion Summary

**Status:** Done
**Branch:** plan/log-text-selection-fix

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/runner.rs` | try_send fallback with direct state mutation; startup toast in `run_with_project` and `run_with_project_and_dap`; exhaustive match replacing `_` catch-all; two new unit tests |

### Notable Decisions/Tradeoffs

1. **Channel-fill strategy in test**: The `test_mouse_capture_changed_channel_full_applies_state_directly` test fills the 256-slot channel with `Message::Tick` entries (a benign no-payload variant) rather than using a mock sender. This avoids introducing test infrastructure while reliably saturating the channel. The direct-mutation path is verified by checking `mouse_capture_active` and the Warn toast level after `handle_runner_actions`.

2. **`run()` demo entry point**: The task says "Apply the same change at all THREE fallback sites". The `run()` function uses `NullClipboard` unconditionally (no `SystemClipboard::new()` attempt), so there is no fallback site to change — no startup toast is added there, matching the task's intent (demo/test mode, not a user-facing entry point).

3. **Exhaustive match listing**: All 44 non-runner `UpdateAction` variants are explicitly enumerated in the match arm. This satisfies the compiler-enforced-awareness goal: adding a new variant will cause a compile error until the developer consciously decides which arm it belongs to.

### Testing Performed

- `cargo test -p fdemon-tui runner` — 5/5 passed (3 existing + 2 new)
- `cargo test --workspace` — all suites passed (0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
- `cargo fmt --all -- --check` — clean
