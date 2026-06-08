# Task 03 — Render the banner outside the New Session Dialog

**Agent:** implementor
**Depends on:** —
**Estimated:** 1.5–2h
**Fixes:** Defect #3b (no render site for the notice outside the dialog)

## Objective

Surface `startup_notice` as a one-line top-row banner on the main/loading screens, so auto-launch
users (who never open the New Session Dialog) see it. Reuse the existing banner formatting so the
copy is identical to the dialog's.

## Files (Write)

- `crates/fdemon-tui/src/render/mod.rs`
- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`

## Files (Read only — DO NOT EDIT)

- `crates/fdemon-app/src/state.rs` (for the `StartupNotice` type + `startup_notice` field)

## Background

- The banner currently renders **only inside** the New Session Dialog widget
  (`render/mod.rs:262-279` matches `Startup | NewSessionDialog`, calling
  `.startup_notice(state.startup_notice.as_ref())` → `render_startup_notice` in the dialog widget
  at `new_session_dialog/mod.rs:655-748`).
- For `UiMode::Loading` and `UiMode::Normal` there is no banner render at all.

## Steps

1. **Extract/expose the banner formatter.** In `new_session_dialog/mod.rs`, make
   `render_startup_notice(notice, area, buf)` (currently private, `:658`) reusable from
   `render/mod.rs`. Either:
   - make it `pub(crate)`, or
   - lift the formatting into a small shared helper (e.g. `pub(crate) fn startup_notice_line(notice: &StartupNotice) -> Line<'_>`)
     that both the dialog and the main-screen render call, guaranteeing identical copy
     (`⬆ New version available: v{latest} (current v{CARGO_PKG_VERSION})`, same style/color).
   Prefer the shared-helper approach to avoid divergence.

2. **Add a top-row render in non-dialog modes.** In `render/mod.rs`, after the existing modal
   match (or in the `Loading`/`Normal` arms / the common header area), when
   `state.startup_notice.is_some()` AND `ui_mode` is NOT `Startup | NewSessionDialog`, draw the
   banner on the **topmost row** of `area` (reserve one line; render the formatter's `Line`).
   - Guard against double-render: only render here when the dialog is not already showing it.
   - Keep layout safe: if the terminal height is too small, skip rendering rather than panicking
     (follow existing area/Rect guards in this file).

3. Confirm the dialog path (`render/mod.rs:270`) is unchanged and still renders the same copy via
   the shared helper.

## Tests

- `startup_notice_renders_on_normal_screen` — build state with `ui_mode = Normal` and a
  `StartupNotice::NewVersionAvailable { latest: "0.5.7" }`, render to a test `Buffer`, assert the
  buffer's top row contains `New version available` and `0.5.7`.
- `startup_notice_renders_on_loading_screen` — same for `UiMode::Loading`.
- `no_banner_when_notice_absent` — `startup_notice = None` → top row does not contain the banner text.
- `no_double_render_in_dialog` — `ui_mode = NewSessionDialog` with a notice renders the banner
  once (inside the dialog), not also as a separate top row.
- Keep the existing `startup_notice_renders_new_version_banner` dialog test
  (`new_session_dialog/mod.rs:1962`) green.

## Acceptance criteria

- [ ] Banner appears on `Normal` and `Loading` screens when `startup_notice` is `Some`.
- [ ] Identical copy/style between dialog and main-screen banner (shared formatter, no duplication).
- [ ] No double-render when the dialog is visible; no panic at small terminal sizes.
- [ ] `cargo test -p fdemon-tui` green; `cargo clippy -p fdemon-tui` clean.

## Out of scope

- Do not edit `state.rs` (Task 02 owns it). Read the `StartupNotice` type only.
- Do not change the notice's lifecycle/clearing (Task 02 handles dismiss-on-keypress).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a5705de4ecb1e0074

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | Added `pub(crate) fn startup_notice_line(notice: &StartupNotice) -> Line<'_>` shared formatter; updated `render_startup_notice` to call it instead of inlining the text |
| `crates/fdemon-tui/src/render/mod.rs` | Imported `StartupNotice`; added `BANNER_MIN_HEIGHT` constant, `should_render_banner_outside_dialog()`, and `render_banner()` helpers; computed `content_area` in `view()` — when notice is `Some` and mode is not `Startup|NewSessionDialog`, reserves top row for banner and shifts all layout by 1; updated all modal `area` references to use `content_area` |
| `crates/fdemon-tui/src/render/tests.rs` | Added 4 tests: `startup_notice_renders_on_normal_screen`, `startup_notice_renders_on_loading_screen`, `no_banner_when_notice_absent`, `no_double_render_in_dialog` |

### Notable Decisions/Tradeoffs

1. **Shared helper approach**: Chose `pub(crate) fn startup_notice_line` over making `render_startup_notice` pub(crate), so the formatting is shared via a `Line`-returning function rather than duplicating the full render logic. Both callers wrap it in a `Paragraph` with the same `STATUS_YELLOW` style and `Alignment::Center`.

2. **`content_area` layout shift**: Rather than rendering the banner as a pure overlay (which would partially obscure the header), we compute a `content_area` that is shrunk by 1 row from the top when the banner is shown. All modal renders (`NewSessionDialog`, `Loading`, `ConfirmDialog`, `Settings`, `FlutterVersion`, `InstallWizard`) use `content_area`. This keeps modals fully within the non-banner area. Toast notifications intentionally still use `area` (full screen) since they render on top of everything.

3. **Double-render prevention**: `should_render_banner_outside_dialog` returns `false` for `Startup | NewSessionDialog`, so the banner is never rendered as a top-row overlay in those modes — the dialog widget renders it internally via the same `startup_notice_line` helper.

4. **`state.rs` untouched**: `StartupNotice` was only read (via `use fdemon_app::state::StartupNotice`). No changes to `state.rs`.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all test suites green, zero failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- `cargo test -p fdemon-tui -- startup_notice no_banner no_double` - All 5 banner-related tests pass

### Risks/Limitations

1. **`content_area` shift applies to all modes with a notice**: If a future mode sets `startup_notice` while in (e.g.) `Settings` mode, the settings panel will be shifted down by 1 row. This is intentional and consistent — the banner always occupies row 0.
2. **`LoadingState::new` ignores its argument**: The test creates a `LoadingState` for the Loading-screen test but the message is random; the test only checks for banner text, which is independent of the loading message.
