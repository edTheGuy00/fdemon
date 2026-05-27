## Task: Animate the new-session dialog discovery & refresh spinners

**Objective**: Replace the dialog's frozen "Discovering devices…" line and static tab-bar refresh glyph with the shared animated spinner, driven by the global `AppState::animation_frame`. All concurrent spinners in the dialog pulse **in phase** (computed from one frame value per render).

**Depends on**: 01-spinner-helper

**Estimated Time**: 1–1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/render/mod.rs`: pass `state.animation_frame` into `NewSessionDialog` at the construction site (`UiMode::Startup | UiMode::NewSessionDialog`, ~line 251).
- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`: add an `animation_frame` field + `.animation_frame(u64)` builder to `NewSessionDialog`; thread it into both `TargetSelector` construction sites (lines ~351 and ~574) and the regions path (~928).
- `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`: add `animation_frame` to `TargetSelector` (builder, default 0); animate `render_loading`; thread the frame into `TabBar`.
- `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs`: accept the frame and render an animated spinner glyph in place of the static refresh icon when `refreshing` / `bootable_refreshing`.

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/spinner.rs`: `spinner_char`, `SPINNER_TICKS_PER_FRAME`.
- `crates/fdemon-app/src/state.rs`: `AppState::animation_frame` (read-only).

### Details

**1. Thread the frame from `render/mod.rs` (~line 251).** Add `.animation_frame(state.animation_frame)` to the builder chain:

```rust
let dialog = widgets::NewSessionDialog::new(
    &state.new_session_dialog_state,
    &state.tool_availability,
    &icons,
)
.startup_notice(state.startup_notice.as_ref())
.enable_mouse(state.settings.ui.enable_mouse)
.animation_frame(state.animation_frame);
```

**2. `NewSessionDialog` (`mod.rs`).** Add the field + builder (model after `enable_mouse`, lines 168/201), default `0`:

```rust
pub struct NewSessionDialog<'a> {
    // ...
    enable_mouse: bool,
    /// Global animation frame (`AppState::animation_frame`) for in-progress
    /// spinners. Defaults to 0 so tests constructing the dialog need no change.
    animation_frame: u64,
}

// in new(): animation_frame: 0,

/// Set the global animation frame used to drive in-progress spinners.
pub fn animation_frame(mut self, frame: u64) -> Self {
    self.animation_frame = frame;
    self
}
```

Pass it through wherever `TargetSelector::new(...)` is built (lines ~351, ~574, ~928) via the new `TargetSelector` builder:

```rust
let target_selector = TargetSelector::new(state, self.tool_availability, is_focused)
    .icons(self.icons.clone())          // existing
    .animation_frame(self.animation_frame);
```

**3. `TargetSelector` (`target_selector.rs`).** Add the field + builder (default 0, model after `enable_mouse`, lines 70–73). Then animate `render_loading` (lines 319–324). Compute the cadence-adjusted index **once** so it matches any other spinner this render:

```rust
fn render_loading(&self, area: Rect, buf: &mut Buffer) {
    let glyph = crate::widgets::spinner::spinner_char(
        self.animation_frame / crate::widgets::spinner::SPINNER_TICKS_PER_FRAME,
    );
    let text = Paragraph::new(format!("{glyph} Discovering devices..."))
        .style(Style::default().fg(palette::STATUS_YELLOW))
        .alignment(Alignment::Center);
    text.render(area, buf);
}
```

Thread the frame into the `TabBar` construction (lines ~115–122) so the refresh indicator animates too — pass `self.animation_frame` to a new `TabBar` parameter/builder.

**4. `TabBar` (`tab_bar.rs`).** Where the refresh label is built (lines ~100–108), when `refreshing`, render the animated spinner glyph instead of (or alongside) the static `icons.refresh()`:

```rust
let label = if refreshing {
    let glyph = crate::widgets::spinner::spinner_char(
        animation_frame / crate::widgets::spinner::SPINNER_TICKS_PER_FRAME,
    );
    format!("{} {glyph}", tab.label())
} else {
    tab.label().to_string()
};
```

Add `animation_frame: u64` to `TabBar` (constructor arg or builder; default 0 in tests). Keep the existing `connected_refreshing` / `bootable_refreshing` gating — the spinner only appears while a refresh is in flight.

**Phase coherence:** both `render_loading` and `TabBar` derive their glyph from the same `self.animation_frame` with the same `SPINNER_TICKS_PER_FRAME` divisor, so they advance in lockstep (the success criterion "concurrent spinners are in phase").

### Acceptance Criteria

1. With the dialog open and `loading == true`, the content line reads `⠋ Discovering devices...` (glyph for the current frame) and the glyph advances as `animation_frame` increments.
2. When `connected_refreshing` (or `bootable_refreshing`) is set, the corresponding tab label shows an animated spinner glyph; when not refreshing, the label is the plain static text (no glyph) — unchanged from today.
3. The discovery-line spinner and tab-bar spinner are derived from a single frame value per render and advance together (in phase).
4. `NewSessionDialog`, `TargetSelector`, and `TabBar` default `animation_frame` to `0`, so existing widget unit tests compile without per-test churn (only the new builder calls are added on the render path).
5. Existing dialog tests (`target_selector.rs::test_target_selector_renders_loading` asserting "Discovering devices") still pass — the substring "Discovering devices" remains present (the glyph is a prefix, not a replacement).
6. `cargo test -p fdemon-tui`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` pass.

### Testing

- Update/extend `target_selector.rs` tests: keep the existing "Discovering devices" substring assertion; add a test that the rendered loading content contains a glyph from `SPINNER_FRAMES` at a known frame.
- Extend `tab_bar.rs` tests (`test_tab_bar_renders_connected_refreshing_indicator`, `..._bootable_..._indicator`, `..._no_indicator_when_not_refreshing`): with a non-zero `animation_frame`, the refreshing tab contains a `SPINNER_FRAMES` glyph; the non-refreshing case contains none.
- A focused test that two spinner glyphs derived from the same frame value are equal (phase coherence), if a convenient seam exists.

### Notes

- **Builder over required param:** add `animation_frame` as a builder defaulting to `0` (not a new positional `new` argument). This keeps the many direct `TargetSelector::new(...)` / `TabBar::new(...)` test constructions compiling and matches the existing `enable_mouse` / `icons` builder convention.
- **Cadence:** use `SPINNER_TICKS_PER_FRAME` (from task 01) so the dialog spinner is calm (~100 ms/frame), distinct from the loading screen's raw per-tick cadence. Do not introduce a second magic divisor.
- **In scope only:** discovery line + tab-bar refresh. Do **not** add spinners to main-view session/phase rows (PLAN "Optional"; deferred to avoid conflicting with the Phase 2/2.5 status-label shimmer).
- **Ordering:** this task edits `render/mod.rs` after task 02. Run on the same branch, after 02 — not in a parallel worktree (see TASKS.md Overlap Matrix).
