## Task: Document Settings vs NewSession dart-defines Edit-pane scroll divergence

**Objective**: Add an inline cross-reference comment at `crates/fdemon-app/src/handler/mouse/settings.rs:21` so the intentional asymmetry between Settings (Edit pane swallows scroll) and NewSessionDialog (Edit pane routes Up/Down) is discoverable from either side. No behavior change in either handler.

**Depends on**: None

**Estimated Time**: 0.25h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mouse/settings.rs`: Replace the existing one-line comment at line 20 (`// Edit pane is text input — wheel must not move the list underneath.`) with a multi-line block that adds a cross-reference to `new_session.rs` and explains the keyboard-handler asymmetry that drives both choices.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/mouse/new_session.rs`: Source of the existing rationale comment at lines 25-26 ("The dart-defines modal handler at keys.rs:851-855 routes Up/Down unconditionally — so do we.").
- `crates/fdemon-app/src/handler/keys.rs`: Reference for the asymmetric keyboard handlers — `handle_key_settings_dart_defines` at `keys.rs:733-770` (only routes Up/Down in List pane) vs `handle_dart_defines_modal_key` at `keys.rs:839-866` (routes Up/Down unconditionally).

### Details

The current comment in `settings.rs:20-21` is brief and doesn't surface the divergence:

```rust
// Edit pane is text input — wheel must not move the list underneath.
DartDefinesPane::Edit => None,
```

The companion comment in `new_session.rs:25-31` documents the opposite choice but does not point to `settings.rs`. A reader on either side has no way to discover the divergence. Replace the `settings.rs` comment with:

```rust
// Edit pane is text input — wheel must not move the list underneath.
//
// Asymmetry note: NewSessionDialog's dart-defines modal routes Up/Down in
// BOTH panes (see `new_session.rs::handle_scroll`). The two surfaces look
// identical to a user but behave differently. The asymmetry mirrors the
// underlying keyboard handlers:
//   - Settings dart-defines (keys.rs:733-770) only binds Up/Down in List pane.
//   - NewSessionDialog dart-defines (keys.rs:839-866) binds Up/Down in both.
// Reconciling the two surfaces requires changing the keyboard handler at
// keys.rs:851-855 — a real product decision, not a polish fix. If pursued,
// see `workflow/plans/bugs/dart-defines-edit-scroll-asymmetry/` (TBD).
DartDefinesPane::Edit => None,
```

No behavior changes. No tests change. No imports change.

### Acceptance Criteria

1. The `DartDefinesPane::Edit` arm in `settings.rs` carries a multi-line comment naming `new_session.rs` and explaining the keyboard-handler-driven asymmetry.
2. The comment explicitly references the relevant `keys.rs` line numbers (`733-770` for Settings, `839-866` and `851-855` for NewSession).
3. No behavior change — `handle_scroll` for Settings dart-defines Edit pane still returns `None`.
4. No new imports, no new tests required (existing `dart_defines_edit_pane_swallows_scroll` test already covers the behavior).
5. `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

No new tests required. Verify:

```bash
cargo test -p fdemon-app handler::mouse::settings
cargo clippy -p fdemon-app --all-targets -- -D warnings
```

The existing `dart_defines_edit_pane_swallows_scroll` test must still pass without modification.

### Notes

- **Why a comment instead of changing behavior.** Reconciling NewSessionDialog's dart-defines Edit pane to also swallow scroll requires updating the keyboard handler at `keys.rs:851-855` so keyboard and mouse stay aligned. That is a behavior change for keyboard users and needs product approval. The comment-only approach in this task closes the documentation gap immediately and leaves the behavioral reconciliation as a separate decision.
- **DO NOT touch `new_session.rs`.** Task 05 owns that file in this phase (for the `_mods` parameter comment). Adding a back-pointer comment here from `new_session.rs` would create a write-file conflict.
- **DO NOT touch `mod.rs` or `keys.rs`.** Out of scope.
- **The `(TBD)` reference** in the comment is intentional — the bug task may or may not be filed depending on user priority. Wording matches `workflow/plans/bugs/<name>/` convention so a future grep finds the cross-reference.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mouse/settings.rs` | Replaced single-line comment on `DartDefinesPane::Edit` arm with 10-line block documenting the keyboard-handler-driven asymmetry, with cross-references to `new_session.rs`, `keys.rs:733-770`, `keys.rs:839-866`, and `keys.rs:851-855`. |

### Notable Decisions/Tradeoffs

1. **Comment-only fix:** Behavior on both surfaces is unchanged. The asymmetry is documented from the Settings side; reconciling behavior is deferred to a future bug task.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo test -p fdemon-app handler::mouse::settings` — Passed (11 tests, including `dart_defines_edit_pane_swallows_scroll`)
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **Asymmetry persists:** Users who switch between Settings dart-defines and NewSessionDialog dart-defines will see different scroll behavior in the Edit pane. The comment makes the intent discoverable but does not eliminate the inconsistency.
