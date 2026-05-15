## Task: terminal.rs `set_mouse_capture` doc-comment correction

**Objective:** Correct the `set_mouse_capture` doc-comment so it accurately describes the disable-path behavior (write errors are logged at `warn` and NOT returned as `Err`). Currently the doc promises error surfacing the implementation cannot deliver.

**Depends on:** None

**Agent:** implementor

**Estimated time:** 15-30 minutes

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/terminal.rs`: Update doc-comment for `set_mouse_capture` (lines 199-201 of the current source).

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/terminal.rs`: same file, examine `disable_mouse_capture()` to confirm it swallows errors internally.
- `workflow/plans/bugs/log-text-selection-broken/BUG.md`: cross-reference for the toggle design.

### Details

Current doc-comment fragment (lines 199-201):

```rust
/// - `enabled = false` → calls [`disable_mouse_capture`] and surfaces any write
///   error as a `Result`, unlike the bare `disable_mouse_capture()` which swallows
///   errors. Returns `Ok(())` without re-emitting DECRST sequences if capture is
///   already off (idempotent via the `MOUSE_CAPTURE_ON` flag).
```

Replace with text that accurately reflects the implementation at lines 220-235:

```rust
/// - `enabled = false` → calls [`disable_mouse_capture`] and **always returns
///   `Ok(())`**. Write errors on the disable path are logged at `warn` level by
///   [`disable_mouse_capture`] but cannot be propagated through this wrapper —
///   the underlying function returns `()`, not `Result`. Callers must not rely on
///   `Err` to detect disable failures; only the enable path surfaces write errors
///   as `Err`. Returns `Ok(())` without re-emitting DECRST sequences if capture is
///   already off (idempotent via the `MOUSE_CAPTURE_ON` flag).
```

The wording deliberately:
- Names the asymmetry (enable surfaces errors, disable doesn't).
- Calls out why (`disable_mouse_capture()` returns `()`).
- Tells callers what NOT to do (don't rely on `Err` for disable detection).

### Acceptance Criteria

1. The doc-comment on `set_mouse_capture` no longer claims the disable path "surfaces any write error as a `Result`".
2. The doc-comment explicitly says disable-path errors are logged but not returned.
3. `cargo doc -p fdemon-tui --no-deps` builds without warnings; the rendered doc-comment is grammatical.
4. No code change — only the doc-comment.

### Testing

No new tests. The change is doc-only. Run `cargo check --workspace` to confirm no syntax issues.

### Notes

- Task 08 will rely on this doc-comment being accurate when implementing the runner-side `try_send` fallback. If a future cleanup wants to make `disable_mouse_capture()` return `Result<()>`, that's a separate refactor (out of scope for this follow-up).
- Do NOT modify the `set_mouse_capture` function body or `disable_mouse_capture`'s signature.
