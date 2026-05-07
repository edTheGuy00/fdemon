## Task: Compact NewSessionDialog — Mouse-Not-Available Hint

**Objective**: When `state.settings.ui.enable_mouse` is `true` and the `TargetSelector` falls back to its compact-vertical layout (40–69 wide × 20–21 tall, the size range deferred from Phase 5 Task 09), render a one-line hint indicating mouse is not registered for device rows at this size. Keyboard interaction is unaffected.

**Depends on**: None

**Estimated Time**: 1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`: in the `render_compact` path (around `fn render_compact` near line 148), insert a hint-line render call when `enable_mouse` is true. Pass the setting through the `TargetSelector` builder (add `with_enable_mouse(bool)` or similar) or read it through state — match whatever pattern the surrounding code uses for other setting reads.
- `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` (tests at the bottom of the same file, or a sibling `tests.rs` if one exists): add a snapshot/render test verifying the hint appears in compact mode with `enable_mouse=true` and is absent with `enable_mouse=false` and absent in non-compact mode.
- Any caller of `TargetSelector::new(...)` in `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` (and possibly `state/...`): pass the new setting through.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs`: `AppState::settings.ui.enable_mouse` accessor path.
- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`: how the compact threshold is decided (where `target_selector.compact(true)` is called); confirm the hint should show only when `compact == true`.
- `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` lines 65–145 (wide path) and 147–280 (compact path) for current layout.

### Details

The compact-vertical render path in `target_selector.rs` does not register `MouseCtx` device-row regions (this was deferred from Phase 5 Task 09 with status `Done ⚠️ Concern`). At terminal sizes that fall into the compact range, mouse users see no clickable device rows — a silent ergonomic dead spot.

The fix: render a small hint when in compact mode and mouse is enabled. Keyboard remains the canonical input at this size.

#### Hint placement

Inside `render_compact`, locate the existing `Layout::vertical(...)` chunk allocation (around line 166). The current chunks likely include: tab bar, device list, footer/status. Reserve one row for the hint either:

- (Preferred) **Above the device list**, between the tab bar and the list — the hint is informational, not status.
- **At the bottom**, replacing or extending an existing footer if one exists.

The author may choose. The hint is a single `Paragraph` line with dim styling (e.g. `Style::default().fg(Color::DarkGray)` matching the project's existing dim-text pattern; check sibling widgets for the canonical color choice).

#### Hint copy

Suggested text (≤ 28 chars to fit narrow terminals):

- `"Resize wider for mouse"`  (22 chars)
- `"⌨ keyboard only at this size"`  (29 chars; uses Unicode keyboard glyph)
- `"Mouse needs ≥ 70 cols"`  (22 chars)

The author picks. Avoid `enable_mouse = false` syntax in the hint — that is a config concept, not the user's immediate problem.

#### Setting plumbing

The simplest path: add a builder method to `TargetSelector`:

```rust
impl<'a> TargetSelector<'a> {
    pub fn enable_mouse(mut self, enable: bool) -> Self {
        self.enable_mouse = enable;
        self
    }
}
```

(plus a `enable_mouse: bool` field defaulted to `false`). The caller in `new_session_dialog/mod.rs` reads `state.settings.ui.enable_mouse` and threads it through.

In `render_compact`:

```rust
fn render_compact(&self, area: Rect, buf: &mut Buffer) {
    // ... existing tab-bar + device-list layout ...

    if self.enable_mouse {
        let hint = Paragraph::new("Resize wider for mouse")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        hint.render(hint_chunk, buf);
    } else {
        // Skip rendering — keep the chunk empty (or shrink the layout).
    }
}
```

If skipping the chunk causes layout instability, allocate the hint row only when `enable_mouse` is true and re-derive chunks accordingly. The author picks the cleaner approach.

#### Edge cases

- **`enable_mouse = false`:** hint is never rendered.
- **Wide layout (`compact == false`):** hint is never rendered (device rows are clickable; no need for the hint).
- **`compact == true` and tabs are at "Bootable" or future tab variants:** hint still renders — the limitation is layout-driven, not tab-driven.
- **Terminal so small that the hint itself overflows:** acceptable degradation; the hint truncates per ratatui's default Paragraph behaviour. No effort to detect-and-skip.

### Acceptance Criteria

1. `TargetSelector` accepts an `enable_mouse: bool` (via builder method or struct field).
2. In `render_compact`, the hint line renders when `enable_mouse` is true; it is absent when `enable_mouse` is false.
3. In the wide render path (`render` non-compact), no hint is rendered (preserves existing visual baseline).
4. A unit test in `target_selector.rs::tests` (or `tests.rs`) renders the widget at three configurations:
   - 60 × 20 (compact), `enable_mouse = true` → hint present.
   - 60 × 20 (compact), `enable_mouse = false` → hint absent.
   - 100 × 30 (wide), `enable_mouse = true` → hint absent.
   The test inspects the rendered `Buffer` for the hint substring.
5. `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` all pass.
6. The hint does not register a clickable region (this task adds no `MouseCtx` calls — region registration for compact-vertical is explicitly out of scope per the Phase 6 plan).
7. Manual smoke test: launch fdemon at 60-column-wide terminal; confirm the hint is visible above (or below) the device list. Resize to 100-column-wide; confirm the hint disappears and device rows are clickable. Set `enable_mouse = false`; confirm no hint at any size.

### Testing

```bash
cargo test -p fdemon-tui target_selector
cargo clippy -p fdemon-tui --all-targets -- -D warnings
```

Snapshot test sketch:

```rust
#[test]
fn test_target_selector_compact_renders_mouse_hint_when_enabled() {
    let state = TargetSelectorState::default();
    let widget = TargetSelector::new(&state).compact(true).enable_mouse(true);
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 20));
    widget.render(buf.area, &mut buf);
    assert!(buf_contains(&buf, "Resize"));  // or whichever hint substring
}

#[test]
fn test_target_selector_compact_no_hint_when_mouse_disabled() {
    let state = TargetSelectorState::default();
    let widget = TargetSelector::new(&state).compact(true).enable_mouse(false);
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 20));
    widget.render(buf.area, &mut buf);
    assert!(!buf_contains(&buf, "Resize"));
}

#[test]
fn test_target_selector_wide_no_hint_even_when_mouse_enabled() {
    let state = TargetSelectorState::default();
    let widget = TargetSelector::new(&state).compact(false).enable_mouse(true);
    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
    widget.render(buf.area, &mut buf);
    assert!(!buf_contains(&buf, "Resize"));
}
```

(The `buf_contains` helper may already exist in the test module; if not, write a small one or scan `buf.content().iter().map(|c| c.symbol())`.)

### Notes

- This task is render-only. It does not register clickable regions for the compact-vertical layout — that is intentionally deferred per the Phase 6 plan.
- Do not add a `Message` variant, a new `UiMode` state, or any handler logic. The hint is pure presentation.
- The hint copy is the author's choice within the constraints listed (concise, neutral, does not include config syntax).
- If you find that `target_selector.rs` already has structural issues that block adding the hint cleanly (e.g. the chunks vector is fixed-size and inflexible), keep the fix minimal — add the hint inside an existing chunk if possible, rather than restructuring the file.
- The hint is **not localized**. The project does not have a localization framework; English-only is the existing convention.
- Do not modify the wide render path. Even if you notice incidental cleanup opportunities, leave them for a separate task.
