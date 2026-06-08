## Task: Encapsulate `startup_notice` Set Path + Doc/Test Cleanup

**Objective**: Route the `startup_notice` set through a named `AppState` method for lifecycle
consistency (C1), enforce the digit-and-dot invariant on `latest` at that boundary (S3), refresh
two stale doc comments (C2, C3), and add the missing `UiMode::Startup` dismiss-no-op test (C4).

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 1–1.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs`: add `set_startup_notice`; refresh the `startup_notice` field
  doc; add the `Startup`-mode dismiss-no-op test.
- `crates/fdemon-app/src/handler/update.rs`: call `set_startup_notice` from the
  `Message::NewVersionAvailable` arm instead of a direct field write.
- `crates/fdemon-app/src/message.rs`: refresh the `NewVersionAvailable` variant rustdoc.

**Files Read (Dependencies):**
- None.

### Details

#### C1 + S3 — Named setter with boundary validation

Today the handler writes the field directly (`handler/update.rs:390`):

```rust
Message::NewVersionAvailable { latest } => {
    state.startup_notice = Some(StartupNotice::NewVersionAvailable { latest });
    ...
}
```

Every other `AppState` lifecycle mutation goes through a named method, and both *clear* paths
(`hide_new_session_dialog:1694`, `dismiss_startup_notice_on_interaction:1704`) already do — the set
path is the lone exception. Add a setter next to the dismiss helper in `state.rs` and centralize the
S3 validation there (single producer path):

```rust
/// Sets the one-line startup notice. This is the single entry point for
/// populating `startup_notice`; the clear paths are `hide_new_session_dialog`
/// and `dismiss_startup_notice_on_interaction`.
///
/// The `latest` carried by `StartupNotice::NewVersionAvailable` is, by contract,
/// the digit-and-dot normalized form produced by `version_check::check_for_newer_release`
/// (see its "returned string is digit-and-dot only" doc). The `debug_assert!` documents and
/// enforces that invariant in debug builds so a future producer cannot smuggle terminal
/// escape sequences into the banner via the public `Message` boundary.
pub fn set_startup_notice(&mut self, notice: StartupNotice) {
    #[cfg(debug_assertions)]
    {
        let StartupNotice::NewVersionAvailable { latest } = &notice;
        debug_assert!(
            latest.chars().all(|c| c.is_ascii_digit() || c == '.'),
            "startup_notice latest must be digit-and-dot only, got {latest:?}"
        );
    }
    self.startup_notice = Some(notice);
}
```

> Adjust the destructuring to match the real `StartupNotice` definition. If `StartupNotice` is a
> single-variant enum, an irrefutable `let` is fine; if more variants exist, `match` and only assert
> for `NewVersionAvailable`. Do **not** change the `StartupNotice` type itself.

Then the handler arm becomes:

```rust
Message::NewVersionAvailable { latest } => {
    state.set_startup_notice(StartupNotice::NewVersionAvailable { latest });
    ...
}
```

Leave the surrounding comment and any follow-up logic in that arm intact.

#### C2 — Refresh `Message::NewVersionAvailable` rustdoc

`message.rs:692-695` still says the notice only makes "the New Session Dialog render a one-line
banner." Update it to reflect the decoupled render path:

```rust
/// A newer fdemon release was discovered on GitHub during the startup
/// background check. Stores the version in `AppState::startup_notice`, which is
/// rendered either above the New Session Dialog (Startup / NewSessionDialog
/// modes) or as a standalone top-row banner on all other screens (Normal,
/// Loading, …). Cleared on the first keypress outside the dialog.
NewVersionAvailable { latest: String },
```

#### C3 — Refresh `startup_notice` field doc

`state.rs:1441-1447` documents only the `hide_new_session_dialog` clear path. Add the second path:

```rust
/// Optional one-line notice rendered on startup — above the New Session Dialog
/// (Startup / NewSessionDialog modes) or as a standalone top-row banner on
/// other screens.
///
/// Set via [`AppState::set_startup_notice`] (e.g. from `Message::NewVersionAvailable`).
/// Cleared either when the New Session dialog is dismissed
/// ([`AppState::hide_new_session_dialog`]) or on the first keypress in a
/// non-dialog mode ([`AppState::dismiss_startup_notice_on_interaction`]).
pub startup_notice: Option<StartupNotice>,
```

#### C4 — `UiMode::Startup` dismiss-no-op test

`is_new_session_dialog_visible()` returns `true` for **both** `NewSessionDialog` and `Startup`
(`state.rs:1764`), but only `NewSessionDialog` has a no-op test
(`dismiss_startup_notice_on_interaction_noop_in_dialog:3127`). Add the `Startup` variant so a future
refactor that compares `ui_mode` directly (instead of calling the helper) cannot silently regress:

```rust
#[test]
fn dismiss_startup_notice_on_interaction_noop_in_startup_mode() {
    let mut state = AppState {
        startup_notice: Some(StartupNotice::NewVersionAvailable { latest: "0.5.7".into() }),
        ..AppState::new()
    };
    state.ui_mode = UiMode::Startup;
    state.dismiss_startup_notice_on_interaction();
    assert!(
        state.startup_notice.is_some(),
        "notice must survive a keypress while the startup splash is visible"
    );
}
```

### Acceptance Criteria

1. `AppState::set_startup_notice` exists, is the only place that assigns `Some(..)` to
   `startup_notice` in production code, and the handler calls it (no direct field write remains in
   `handler/update.rs`).
2. In debug builds, `set_startup_notice` asserts that a `NewVersionAvailable` `latest` is
   digit-and-dot only; release builds are unaffected (no runtime cost).
3. The `Message::NewVersionAvailable` rustdoc and the `startup_notice` field doc both describe the
   dialog-and-standalone render path plus both clear paths.
4. A new `dismiss_startup_notice_on_interaction_noop_in_startup_mode` test passes; the existing
   dialog no-op, normal-clear, and no-notice tests still pass.
5. `cargo test -p fdemon-app` green; `cargo clippy -p fdemon-app --all-targets -- -D warnings`
   clean; `cargo fmt --all -- --check` clean.

### Notes

- Do NOT modify the `StartupNotice` enum definition, the dismiss/visibility predicates, or the
  unconditional-store behavior (storing in any `ui_mode` is the Defect-#3 fix and must remain).
- The `debug_assert!` is documentation-as-code; it must not change release behavior. Do not convert
  it to a runtime `if`/early-return — the value is always valid from the real producer.
- Keep the existing handler tests (`new_version_available_sets_startup_notice_*`) green; they should
  pass unchanged since behavior is identical.

---

## Completion Summary

**Status:** _pending_
