## Task: Adopt the shared spinner in the loading screen

**Objective**: Replace the inline `SPINNER` constant in `render_loading_screen` with the `widgets::spinner` helper from task 01, with **zero visual change** to the startup screen.

**Depends on**: 01-spinner-helper

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/render/mod.rs`: `render_loading_screen` (lines ~521–527).

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/spinner.rs`: the helper.
- `crates/fdemon-tui/src/render/snapshots/flutter_demon__tui__render__tests__loading.snap`: verify it still matches (frame 0 → `⠋`).

### Details

Today (`render/mod.rs:521–527`):

```rust
fn render_loading_screen(frame: &mut Frame, state: &AppState, loading: &LoadingState, area: Rect) {
    // Braille spinner characters for smooth animation
    const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner_idx = (loading.animation_frame as usize) % SPINNER.len();
    let spinner_char = SPINNER[spinner_idx];
    // ...
    Span::styled(spinner_char, /* style */),
```

Replace with the shared helper, **passing `loading.animation_frame` unchanged** (no divisor) to preserve the exact one-glyph-per-tick cadence:

```rust
fn render_loading_screen(frame: &mut Frame, state: &AppState, loading: &LoadingState, area: Rect) {
    // ...
    let glyph = crate::widgets::spinner::spinner_char(loading.animation_frame);
    // ...
    Span::styled(
        glyph.to_string(),
        Style::default().fg(palette::ACCENT).add_modifier(Modifier::BOLD),
    ),
```

Notes on the swap:
- The previous code stored `&str`; `spinner_char` returns `char`. The `Span::styled` content needs `glyph.to_string()` (owned) — adjust the span construction accordingly.
- Keep the surrounding layout, colors (`palette::ACCENT`, `Modifier::BOLD`), and message span exactly as they are.
- `SPINNER_FRAMES[0]` is `⠋`, so frame 0 renders identically to today.

### Acceptance Criteria

1. `render_loading_screen` no longer declares a local `SPINNER` constant; it calls `widgets::spinner::spinner_char`.
2. The loading-screen glyph, cadence (one frame per tick via `loading.animation_frame`), color, and layout are unchanged.
3. The `*loading.snap` snapshot test passes unchanged (frame 0 shows `⠋`). If `cargo insta`/snapshot review reports a diff, the diff must be byte-identical/empty — do **not** re-bless a changed glyph.
4. `cargo test -p fdemon-tui` passes (including `render::tests`).

### Testing

- `cargo test -p fdemon-tui` — existing `render` snapshot/transition tests cover the loading screen; confirm the `loading.snap` test still passes.
- Optional: a focused assertion that `render_loading_screen` output at `animation_frame == 0` contains `⠋` (likely already covered by the snapshot).
- `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings`.

### Notes

- This is the "no visual regression" success criterion — the point is purely to dedupe the glyph set onto the shared helper, not to change behavior. Resist any cadence/cosmetic tweak here; the dialog (task 03) is where new animation appears.
- Both this task and task 03 edit `render/mod.rs`. Run **02 before 03 on the same branch** — do not run them in parallel worktrees (see TASKS.md Overlap Matrix).
