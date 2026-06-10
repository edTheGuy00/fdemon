## Task: TUI — Windows leaf caption + coming-soon suppression

**Objective**: Give the Windows leaf its detail-pane caption and stop the "coming soon" action hint
from rendering alongside a populated guided-command block (the dual-CTA bug Web/iOS/macOS already
avoid). `step_detail.rs` only — `step_list.rs` renders leaves generically and needs no change.

**Depends on**: Task 03 (merged — the Windows leaf carries real status/components/guided commands, so
rendering tests are meaningful).

**Agent:** implementor

**Complexity:** low

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/types.rs` — `WizardStepKind`.
- `crates/fdemon-app/src/install_wizard/state.rs` — `WizardStep` shape for test fixtures.

### Details

> Line numbers are a current snapshot and will drift — locate by symbol/test-name.

1. **`step_caption()`** (~`step_detail.rs:90`) — replace the `_ => None` fallthrough for Windows with
   an explicit arm (and drop the "Add a new leaf caption here…" reminder comment if Windows was the
   last outstanding leaf):
   ```rust
   WizardStepKind::PlatformWindows =>
       Some("  Visual Studio C++ workload required for Windows desktop builds"),
   ```
   Match the existing captions' two-space indent + sentence style.
2. **`render_action_hint()`** (~`step_detail.rs:268`) — add `WizardStepKind::PlatformWindows` to the
   `matches!` list that suppresses the "coming soon" hint when the step has guided commands (currently
   `PlatformAndroid | Prerequisites | PlatformWeb | PlatformIos | PlatformMacos`).
3. **No change** to `is_executable()` (Windows correctly falls through to `false`) or
   `action_hint_text()` (never called for non-executable kinds).

### Acceptance Criteria

1. The Windows leaf renders its caption in the detail pane.
2. A Windows leaf with guided commands renders the guided block + `c`-copy hint and **no**
   "coming soon" text; with no guided commands (status `Ok`), the display-only hint behaviour matches
   the iOS/macOS leaves.
3. `cargo test -p fdemon-tui --lib` green; `cargo fmt --all` + `cargo clippy --workspace -- -D warnings`
   clean.

### Testing

```bash
cargo test -p fdemon-tui --lib widgets::install_wizard
cargo fmt --all && cargo clippy --workspace -- -D warnings
```

New tests (mirror the existing iOS/macOS/Web ones, ~`step_detail.rs:2074`):
- `test_step_caption_windows_returns_some`
- `test_step_detail_suppresses_coming_soon_for_windows_with_guided_commands`
- `test_step_detail_windows_without_guided_commands_shows_no_dual_cta` (Ok-status leaf)

### Notes

- `step_list.rs` needs nothing: rows render generically from `indent` + `status_glyph`.
- Keep the caption wording consistent with Task 03's leaf title (`"Windows"`) and the guided-command
  labels — the reviewer will read them side by side.
