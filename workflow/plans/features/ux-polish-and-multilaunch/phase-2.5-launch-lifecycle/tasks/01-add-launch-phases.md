## Task: Add `Preparing` + `Launching` phases & display mapping

**Objective**: Add the two new `AppPhase` variants and wire up their *display* (icon, label, color) so the workspace compiles again. This is a pure foundation task: it adds the variants and fixes the two exhaustive `match` sites, but does **not** wire any lifecycle transition yet (tasks 02/03 do that). After this task the new variants exist and render correctly *if* set, but nothing sets them.

**Depends on**: None

**Estimated Time**: 1h

### Scope

**Files Modified (Write):**
- `crates/fdemon-core/src/types.rs`: add `Preparing` and `Launching` to the `AppPhase` enum.
- `crates/fdemon-tui/src/theme/styles.rs`: add match arms to `phase_indicator` (compile fix #1) and update the `test_phase_indicator_all_phases_covered` coverage array.
- `crates/fdemon-app/src/session/session.rs`: add match arms to `status_icon` (compile fix #2) **only** — do not touch `mark_started`/predicates here (task 02 owns those).

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/theme/palette.rs`: `STATUS_BLUE` (`Rgb(56, 189, 248)`) is unused by any phase today — use it for both new phases.

### Details

**1. Enum variants** (`crates/fdemon-core/src/types.rs`, the `AppPhase` enum ~line 13). Insert the two variants between `Initializing` and `Running` so the declaration reads as a launch progression. Keep `#[default]` on `Initializing`.

```rust
pub enum AppPhase {
    /// Application is initializing (created, before any spawn work)
    #[default]
    Initializing,
    /// Waiting for pre-app native-log sources to pass their `ready_check`
    /// (e.g. `start_before_app` HTTP health poll) before the Flutter
    /// process is spawned.
    Preparing,
    /// Flutter process has attached and is building/compiling/first-running;
    /// not yet confirmed up (the `app.started` daemon event flips to Running).
    Launching,
    /// Flutter app is actually running
    Running,
    /// Application is reloading
    Reloading,
    /// Application has stopped
    Stopped,
    /// Application is shutting down
    Quitting,
}
```

**2. `phase_indicator`** (`crates/fdemon-tui/src/theme/styles.rs`, ~line 131). Add two arms. Use `STATUS_BLUE` for both; give `Launching` BOLD (it's "almost live") and `Preparing` no modifier:

```rust
AppPhase::Preparing => (
    icons.circle(),
    "Preparing",
    Style::default().fg(palette::STATUS_BLUE),
),
AppPhase::Launching => (
    icons.circle(),
    "Launching",
    Style::default().fg(palette::STATUS_BLUE).add_modifier(Modifier::BOLD),
),
```

Update the hand-enumerated coverage array in `test_phase_indicator_all_phases_covered` (~line 308) to include `AppPhase::Preparing` and `AppPhase::Launching` so the test stays meaningful.

**3. `status_icon`** (`crates/fdemon-app/src/session/session.rs`, ~line 605). Add two arms only:

```rust
AppPhase::Preparing => "◌",
AppPhase::Launching => "◐",
```

### Acceptance Criteria

1. `AppPhase` has `Preparing` and `Launching` variants; `Initializing` remains `#[default]`.
2. `cargo check --workspace --all-targets` passes (both exhaustive matches updated).
3. `phase_indicator(AppPhase::Preparing, …)` and `phase_indicator(AppPhase::Launching, …)` return `STATUS_BLUE`-fg styles with the labels "Preparing" / "Launching"; `Launching` carries `Modifier::BOLD`.
4. `status_icon` returns distinct glyphs for both new variants.
5. `test_phase_indicator_all_phases_covered` includes the two new variants.
6. No lifecycle transition is wired (no `= AppPhase::Preparing/Launching` assignment anywhere yet); existing tests still pass unchanged.

### Testing

- Add a per-variant assertion in `theme/styles.rs` tests confirming label + `STATUS_BLUE` fg for `Preparing` and `Launching` (mirror the existing per-phase tests).
- `cargo test -p fdemon-tui`, `cargo test -p fdemon-core`, `cargo test -p fdemon-app` (status_icon test, if present).

### Notes

- `AppPhase` has **no serde derive**, so adding variants has zero wire-format/snapshot impact.
- Keep this task strictly additive on `session.rs` (status_icon arms only). Task 02 modifies the same file (`mark_started`, predicates, new field), so leave those untouched to keep the diffs cleanly sequenced.
- If clippy flags the new variants as never-constructed, that is expected at this stage — they are constructed in tasks 02/03. Do **not** add `#[allow(dead_code)]` to enum variants (they are `pub` and part of the public domain type, so clippy will not flag them).

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/types.rs` | Added `Preparing` and `Launching` variants to `AppPhase` enum, between `Initializing` and `Running`, with doc comments |
| `crates/fdemon-tui/src/theme/styles.rs` | Added `Preparing` and `Launching` match arms to `phase_indicator` (STATUS_BLUE fg; Launching gets BOLD); added two new per-variant tests; updated `test_phase_indicator_all_phases_covered` array |
| `crates/fdemon-app/src/session/session.rs` | Added `Preparing => "◌"` and `Launching => "◐"` arms to `status_icon` match only |

### Notable Decisions/Tradeoffs

1. **Strict task scope**: Only modified `status_icon` arms in `session.rs`, leaving `is_active`, `is_running`, and other predicates untouched so task 02 can cleanly own those changes.
2. **No lifecycle wiring**: `Preparing` and `Launching` are never assigned anywhere in this changeset — they render correctly if set, but nothing sets them yet.
3. **Clippy clean**: `pub` enum variants are not flagged by clippy as dead code even when never constructed, so no `#[allow(dead_code)]` annotations were needed.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (6,303 tests across all crates, 0 failed)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **No lifecycle transitions yet**: The new phases exist in the type system and render correctly, but tasks 02/03 must wire the actual state machine transitions before they become observable at runtime.
