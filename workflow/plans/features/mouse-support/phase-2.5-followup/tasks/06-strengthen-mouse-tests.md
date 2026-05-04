## Task: Document `assert_scroll_routes_to` discriminant limitation + add `scroll_during_reload` test

**Objective**: Two changes to `crates/fdemon-app/src/handler/tests.rs`: (1) add a doc comment on the `assert_scroll_routes_to` helper warning that it compares discriminants only, so future callers know to use `matches!` directly for data-carrying `Message` variants; (2) add a test that drives `update(state_with_busy_session, Message::Mouse(Scroll{..}))` and asserts the scroll message still fires — locking in the documented "scroll is never blocked by reload state" invariant from the parent PLAN.md and `keys.rs:263`.

**Depends on**: None

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/tests.rs`:
  1. Add a `///`-style block comment above `fn assert_scroll_routes_to` (around line 10220) explaining that it compares `std::mem::discriminant` only.
  2. Add a new test `scroll_during_reload_does_not_block` (or similarly named) inside the `mod mouse_scroll` block.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/scroll.rs` (or wherever `Message::ScrollUp` is dispatched): Confirm scroll is not blocked by `is_busy` — the `keys.rs:263` "always allowed" invariant.
- `crates/fdemon-app/src/handler/mouse/mod.rs`: Confirm the dispatcher does not gate on session state.
- Existing `mouse_scroll` integration tests at `tests.rs:10203+`: Match the existing test style (`AppState::new()` + `state.ui_mode = ...` + assertion via `assert_scroll_routes_to` or direct `update()` call).

### Details

#### Sub-task A — Document `assert_scroll_routes_to` limitation

Today the helper at `tests.rs:10220` reads:

```rust
fn assert_scroll_routes_to(
    state: &mut AppState,
    dir: ScrollDir,
    mods: KeyModSet,
    expected: Message,
) {
    let result = update(state, Message::Mouse(scroll_input(dir, mods)));
    match result.message {
        Some(actual) => assert!(
            std::mem::discriminant(&actual) == std::mem::discriminant(&expected),
            "expected {:?}, got {:?}",
            expected,
            actual
        ),
        None => panic!("expected Some({:?}), got None", expected),
    }
    assert!(result.action.is_none(), "scroll must not produce an action");
}
```

`std::mem::discriminant` compares the outer variant only. A test that expected `Message::NetworkNavigate(NetworkNav::Up)` would also pass if the dispatcher returned `Message::NetworkNavigate(NetworkNav::PageDown)` because both share the same outer discriminant. Today this is safe because callers needing inner-variant precision use `matches!` directly. To prevent future misuse, prepend a comment:

```rust
/// Assert that `update()` on a `Message::Mouse(Scroll{..})` produces a follow-up
/// message whose **discriminant** matches `expected`.
///
/// IMPORTANT: This helper compares `std::mem::discriminant` only — it does NOT
/// check inner variant data. For `Message` variants that carry a payload (e.g.
/// `NetworkNavigate(NetworkNav::Up)` vs `NetworkNavigate(NetworkNav::PageDown)`)
/// or `DevToolsInspectorNavigate(InspectorNav::Up)` vs `(InspectorNav::Collapse)`,
/// use `matches!(result.message, Some(Message::X(Y)))` directly in the test
/// body. This helper is appropriate only for unit-style variants like
/// `ScrollUp`, `ScrollDown`, `PageUp`, `PageDown`, `SettingsPrevItem`, etc.
fn assert_scroll_routes_to(
    state: &mut AppState,
    dir: ScrollDir,
    mods: KeyModSet,
    expected: Message,
) {
    ...
}
```

#### Sub-task B — `scroll_during_reload` test

The parent PLAN.md and `keys.rs:263` document scroll as "always allowed" — never gated by reload/restart state. Phase 2 inherited this invariant but no test exercises it. Add:

```rust
/// Verifies the documented invariant from `keys.rs:263` and PLAN.md:
/// scroll is never blocked by reload/restart state. If a future change adds
/// an `is_busy` gate to the scroll path (or to `Message::ScrollUp` handling),
/// this test will fail and force re-verification of the safety claim.
#[test]
fn scroll_during_reload_does_not_block() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Normal;

    // Put the selected session into a busy/reloading state. The exact field
    // depends on the SessionHandle API — look for `is_busy()` or the underlying
    // reload-in-progress flag (e.g. `session.lifecycle = SessionLifecycle::Restarting`).
    // If no session is selected, scroll is trivially unblocked, so this test
    // requires a selected session to exercise the busy path.
    //
    // Use the existing test helper for session construction (see `test_device()`
    // and the session-add patterns elsewhere in this file).
    // ... wire a busy session ...

    let result = update(
        &mut state,
        Message::Mouse(scroll_input(ScrollDir::Up, KeyModSet::NONE)),
    );

    assert!(
        matches!(result.message, Some(Message::ScrollUp)),
        "scroll must fire even with a busy session — got {:?}",
        result.message
    );
    assert!(
        result.action.is_none(),
        "scroll must not produce an action even when busy"
    );
}
```

The implementor must wire a busy session using the existing `tests.rs` helpers. Look for examples of `session.is_busy()` or `SessionLifecycle::Restarting` / `Reloading` in test code already in `tests.rs` to find the right construction pattern. If no helper exists, the simplest path is to construct a `SessionHandle` and set whatever flag `is_busy()` checks.

If wiring a busy session turns out to require significant new test scaffolding (more than ~30 LOC), consider downgrading the test to a comment-only TODO and filing a separate task. Document the decision in the Completion Summary.

### Acceptance Criteria

1. `assert_scroll_routes_to` carries a `///` doc comment that names `std::mem::discriminant`, identifies the limitation (no inner-variant check), and recommends `matches!` for data-carrying variants.
2. The doc comment names at least two example variants where the helper is unsafe (e.g., `NetworkNavigate`, `DevToolsInspectorNavigate`).
3. A new test `scroll_during_reload_does_not_block` (or similarly named) exists in `mod mouse_scroll`, drives `update()` with a busy-session `AppState`, and asserts the scroll message still fires.
4. The test passes: `cargo test -p fdemon-app handler::tests::mouse_scroll::scroll_during_reload`.
5. `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` all pass.
6. No production code is touched. Tests-only file changes.

### Testing

```bash
cargo test -p fdemon-app handler::tests::mouse_scroll
cargo test --workspace
```

### Notes

- **Why bundle these two changes.** Both are tests-file-only, both are docstring/test additions (no behavior change), and bundling avoids over-fragmenting the orchestrator wave for two small items. They share the same `tests.rs` worktree.
- **DO NOT touch `mod.rs`** — Task 04 owns it. The positive-assertion tests for Settings and NewSessionDialog go in `mod.rs::tests`, not in `tests.rs::mouse_scroll`.
- **DO NOT touch any `mouse/*.rs` submodule.** Tasks 01, 02, 05 own those.
- **If the busy-session wiring is non-trivial**, the implementor has discretion to leave a `// TODO(phase-3): wire busy session helper` comment and downgrade the test to a structural skeleton. Document the trade-off in the Completion Summary so future work can pick it up. The doc-comment sub-task (A) must still ship in this task.
- **Why not extract `assert_scroll_routes_to` to a shared test_helpers module.** Out of scope for Phase 2.5. The helper currently lives inside `mod mouse_scroll` as a local helper; promoting it to a shared module is a refactor for a later cleanup wave (review Minor #11 also defers `test_device()` hoisting for the same reason).

---

## Completion Summary

**Status:** <!-- Done / Blocked / Failed -->
**Branch:** <!-- current branch name -->

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/tests.rs` | Added doc comment on `assert_scroll_routes_to`; added `scroll_during_reload_does_not_block` test |

### Notable Decisions/Tradeoffs

1. **Busy-session wiring approach:** <!-- describe how the test constructed a busy session, or note if downgraded to TODO -->

### Testing Performed

- `cargo fmt --all -- --check` — Passed/Failed
- `cargo test -p fdemon-app handler::tests::mouse_scroll` — Passed/Failed (X tests)
- `cargo test --workspace` — Passed/Failed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed/Failed

### Risks/Limitations

1. **Test depends on existing busy-session helpers.** If those helpers are absent, the `scroll_during_reload` test may need to be downgraded to a comment-only TODO (decision documented above).
