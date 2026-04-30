# Task 04 — Promote migration log to `warn!` + add TUI banner above New Session dialog

**Plan:** [../BUG.md](../BUG.md) · **Index:** [../TASKS.md](../TASKS.md)
**Agent:** implementor
**Depends on:** Task 01 (helper exists with `bool` return signature)
**Wave:** 2 (parallel with Task 02)

## Goal

Resolve review finding **M2** (migration `tracing::info!` is invisible to most users — file-only, low severity).

Per the locked-in decision (BUG.md §Decisions §7 — "both promotions"), implement two complementary visibility changes:

1. **Promote** the helper's `tracing::info!` → `tracing::warn!`. Trivial change inside Task 01's helper.
2. **Add** a one-line banner above the New Session dialog in TUI mode when the migration nudge applied this process. Banner shows on first dialog appearance per process; clears on dialog dismissal or `ui_mode` change.

## Files Modified (Write)

| File | Change |
|------|--------|
| `crates/fdemon-app/src/config/mod.rs` | Inside the `emit_migration_nudge` helper from Task 01, change both `tracing::info!` arms to `tracing::warn!`. Single keyword swap. |
| `crates/fdemon-app/src/state.rs` (or wherever `AppState` is defined — verify with `grep -rn "pub struct AppState"`) | Add `pub show_migration_banner: bool` field to `AppState`; default `false` in `Default for AppState` and any constructors. |
| `crates/fdemon-tui/src/startup.rs` | Replace Task 01's `let _migration_applied = emit_migration_nudge(...)` with `let migration_applied = emit_migration_nudge(...)`; then `state.show_migration_banner = migration_applied;` before the function returns (apply only when the function returns `StartupAction::Ready` — i.e., when the dialog is actually shown). |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` (verify path with `grep -rn "new_session_dialog"`) | Render a one-line banner above the dialog frame when `state.show_migration_banner == true`. Banner copy: *"⚠ Cache-driven auto-launch is now opt-in. Set `[behavior] auto_launch = true` in `.fdemon/config.toml` to restore."* (Implementor may adjust wording for terminal width / style consistency.) |
| `crates/fdemon-tui/src/widgets/new_session_dialog/...` OR a state handler | Clear `state.show_migration_banner = false` when the dialog is dismissed (user picks a device, presses `Esc`, or `ui_mode` transitions away from `UiMode::Startup`). Implementor's choice of clear-trigger; document in Completion Summary. |

## Files Read (dependency)

- `crates/fdemon-app/src/config/mod.rs` (Task 01's helper signature — read to confirm the `bool` return contract is intact)
- `crates/fdemon-app/src/state.rs` (read existing `AppState` shape before adding field)
- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` (read existing dialog rendering to find the right insertion point for the banner)

## Implementation Notes

### Promote `info!` → `warn!`

Inside the helper from Task 01, change:

```rust
EMITTED.get_or_init(|| match mode {
    NudgeMode::Tui => tracing::info!(...),
    NudgeMode::Headless => tracing::info!(...),
});
```

to:

```rust
EMITTED.get_or_init(|| match mode {
    NudgeMode::Tui => tracing::warn!(...),
    NudgeMode::Headless => tracing::warn!(...),
});
```

Single keyword swap. The text strings stay as defined in Task 01.

### Add `show_migration_banner` field to `AppState`

Verify the path with:

```bash
grep -rn "pub struct AppState" crates/fdemon-app/src/
```

Then add the field with a doc comment explaining its purpose:

```rust
/// Set to `true` when `emit_migration_nudge` reported that the cache-auto-launch
/// migration condition applies. Drives a one-line banner above the New Session
/// dialog so users see the change without needing to inspect the log file.
/// Cleared when the dialog is dismissed or `ui_mode` transitions away from
/// `UiMode::Startup`.
pub show_migration_banner: bool,
```

Update `Default for AppState` (and any other constructor — `AppState::new()`, etc.) to set `show_migration_banner: false`.

### Set the flag in `startup_flutter`

Modify the Task 01 call:

```rust
// Task 01:
let _migration_applied = emit_migration_nudge(NudgeMode::Tui, project_path, settings);

// Task 04 replaces with:
let migration_applied = emit_migration_nudge(NudgeMode::Tui, project_path, settings);

// ... existing logic to compute startup action ...

if has_auto_start_config || cache_trigger {
    return StartupAction::AutoStart { configs };
}

// Default: show NewSessionDialog at startup
state.show_new_session_dialog(configs);
state.ui_mode = UiMode::Startup;
state.show_migration_banner = migration_applied;  // <-- only when dialog actually shows
StartupAction::Ready
```

> Important: only set `show_migration_banner = true` when the dialog is actually displayed (the `Ready` return path). On the `AutoStart` path, the dialog is never shown, so the banner state is irrelevant.

### Render the banner

In the New Session dialog widget, locate the rendering function (likely `fn render(&mut self, area: Rect, buf: &mut Buffer, state: &AppState)` or similar). Above the existing dialog frame, render a one-line banner if `state.show_migration_banner`:

```rust
if state.show_migration_banner {
    // Reserve top row for banner; render dialog in remaining area
    let layout = Layout::vertical([
        Constraint::Length(1),  // banner
        Constraint::Min(0),     // dialog
    ]).split(area);

    let banner = Paragraph::new(
        "⚠ Cache-driven auto-launch is now opt-in. Set `[behavior] auto_launch = true` in `.fdemon/config.toml` to restore."
    )
    .style(Style::default().fg(Color::Yellow))
    .alignment(Alignment::Center);
    banner.render(layout[0], buf);

    // Render dialog in layout[1] instead of area
    self.render_dialog(layout[1], buf, state);
} else {
    self.render_dialog(area, buf, state);
}
```

Adjust to match the existing widget's code style. Verify with `cargo run -- example/app2` that the banner renders without breaking the dialog's responsive layout (per `docs/CODE_STANDARDS.md` §Responsive Layout Guidelines — use `Constraint::Length(1)` for the banner row, `Constraint::Min(0)` for the dialog absorber).

### Clear the banner

The banner should clear so it doesn't persist forever once the user has seen it. Pick one or more clear-triggers:

- **(a)** When the user dismisses the dialog (selects a device, presses `Esc`, opens a different mode). Hook into the existing dismiss handler in the dialog's update logic.
- **(b)** When `ui_mode` transitions from `UiMode::Startup` to anything else. Hook into the state transition.
- **(c)** Both of the above (defense-in-depth).

Implementor's call. Document the chosen trigger in the Completion Summary.

### Tests

- Unit test: assert `AppState::default().show_migration_banner == false`.
- Unit test: simulate `startup_flutter` with the migration condition met (cache + no `auto_launch` + no `auto_start` config); assert `state.show_migration_banner == true` after the call returns `Ready`.
- Unit test: simulate `startup_flutter` with the migration condition NOT met (e.g., cache + `auto_launch = true`); assert `state.show_migration_banner` remains `false`.
- Optional snapshot/render test for the dialog widget with `show_migration_banner = true`. If the existing widget tests use a render-snapshot pattern, follow it; otherwise skip.

## Verification

- `cargo check --workspace --all-targets`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- Manual smoke (TUI):
  1. In `example/app2` with cache + no `auto_launch` + no `auto_start`, run `fdemon`.
  2. New Session dialog appears with the migration banner above it.
  3. The fdemon log file shows the migration message at `WARN` level (not `INFO`).
  4. Pick a device or press `Esc` → banner clears.
  5. Re-trigger the dialog (if possible without restart) → banner does NOT re-show (already seen this process).
- Manual smoke (headless):
  1. Run `fdemon --headless` → migration log line appears at WARN level (visible if the user has WARN-level subscribers).

## Acceptance

- [ ] Helper in `crates/fdemon-app/src/config/mod.rs` emits `tracing::warn!` (not `info!`) for both `NudgeMode::Tui` and `NudgeMode::Headless`.
- [ ] `AppState` has `pub show_migration_banner: bool` with `Default` set to `false`.
- [ ] `startup_flutter` sets `state.show_migration_banner = true` only on the `Ready` (dialog-shown) path when the migration condition applied.
- [ ] New Session dialog renders a one-line banner above the dialog when `state.show_migration_banner == true`.
- [ ] Banner clears on dialog dismissal or `ui_mode` transition (implementor's choice — document in Completion Summary).
- [ ] All existing tests pass; new tests cover the field default, the `Ready`-path setting, and the non-applicable case.
- [ ] `cargo clippy --workspace -- -D warnings` clean.
- [ ] Manual smoke confirms WARN-level log + banner appearance in TUI.
