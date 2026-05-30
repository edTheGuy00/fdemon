## Task: Tint the header background on reload success

**Objective**: When the selected session has a fresh reload flash
(`reload_flash_alpha > 0`), blend the main header background from `CARD_BG`
toward `STATUS_GREEN` using the **existing** Phase 2 `lerp_color` helper, so a
successful hot reload briefly pulses the header green and fades back. Driven by
the existing tick loop — no new timer.

**Depends on**: Task 01 (`Session::reload_flash_alpha`)

**Estimated Time**: 0.5–1h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/header.rs`: add a `reload_flash: f32` field to
  `MainHeader` with a `.reload_flash(alpha)` builder (default `0.0`); in
  `render_main_header`, compute the block bg via `lerp_color` instead of the bare
  `CARD_BG` constant. Add a render test.
- `crates/fdemon-tui/src/render/mod.rs`: compute the alpha from the selected
  session (`handle.session.reload_flash_alpha(Local::now())`) and pass it into the
  `MainHeader` builder.

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/shimmer.rs`: `lerp_color` (already exists from
  Phase 2 — import, do **not** modify).
- `crates/fdemon-tui/src/theme/palette.rs`: `CARD_BG`, `STATUS_GREEN`.
- `crates/fdemon-app/src/session/session.rs`: `reload_flash_alpha` (task 01).

### Details

**1. `MainHeader` carries the alpha (header.rs).** Add the field + builder so the
header stays pure (no `Local::now()` in the TUI widget):

```rust
pub struct MainHeader<'a> {
    project_name: Option<&'a str>,
    session_manager: Option<&'a SessionManager>,
    icons: IconSet,
    reload_flash: f32, // 0.0 = no flash; set via `.reload_flash(..)`
}

impl<'a> MainHeader<'a> {
    // in `new(..)`, initialize `reload_flash: 0.0`

    /// Tint the header background toward the success green by this `0.0..=1.0`
    /// reload-flash intensity (see `Session::reload_flash_alpha`).
    pub fn reload_flash(mut self, alpha: f32) -> Self {
        self.reload_flash = alpha;
        self
    }
}
```

**2. Blend the block bg (header.rs `render_main_header`, ~line 77).** Replace the
hard-coded background:

```rust
/// Peak blend toward the success green at full flash. Kept well below 1.0 so the
/// header tints (not floods) green — readability of title/tabs is preserved.
const RELOAD_FLASH_BLEND_CAP: f32 = 0.35;

let bg = crate::widgets::shimmer::lerp_color(
    palette::CARD_BG,
    palette::STATUS_GREEN,
    header.reload_flash * RELOAD_FLASH_BLEND_CAP,
);
let block = styles::glass_block(false).style(Style::default().bg(bg));
```

When `reload_flash == 0.0`, `lerp_color(.., t = 0.0)` returns `CARD_BG`
unchanged — zero visual change in the steady state, and `lerp_color` already
degrades gracefully on non-RGB terminals.

**3. Plumb the alpha (render/mod.rs, ~line 168).** The header is built just before
the `StatusInfo` block that already borrows the selected session. Read the alpha
from the selected session (if any) and pass it in:

```rust
let reload_flash = state
    .session_manager
    .selected()
    .map(|h| h.session.reload_flash_alpha(chrono::Local::now()))
    .unwrap_or(0.0);
let header = widgets::MainHeader::new(state.project_name.as_deref(), icons)
    .with_sessions(&state.session_manager)
    .reload_flash(reload_flash);
```

Watch the existing borrow flow: the header is rendered (line 171) *before* the
`selected_mut()` borrow at line 177, so an immutable `selected()` read here is
fine. Confirm `chrono` is reachable from `fdemon-tui` (it is used elsewhere via
`fdemon-app` re-exports / its own dep) — otherwise compute the alpha in a small
`let` using whatever `Local`/`now` accessor the render module already has.

### Acceptance Criteria

1. With `reload_flash = 0.0`, the rendered header background equals `CARD_BG`
   exactly (no regression to existing header snapshot/style tests).
2. With `reload_flash = 1.0`, the header background is a blend strictly between
   `CARD_BG` and `STATUS_GREEN` (i.e. tinted, not fully green, due to the cap).
3. The alpha is sourced from the selected session's `reload_flash_alpha`; with no
   selected session the header renders with no tint.
4. A successful hot reload visibly tints the header green and fades within
   ~500 ms in a manual run (driven by the existing 50 ms tick redraws).
5. No new timer, no new `AppState`/`Session` field, no duplicated duration/RGB
   math (reuses task 01's helper and Phase 2's `lerp_color`).

### Testing

Add a header render test (model after the existing `header.rs` `#[cfg(test)]`
tests that build a `MainHeader` and render into a test buffer):

```rust
#[test]
fn header_bg_unchanged_without_flash() {
    // MainHeader::new(..).reload_flash(0.0) → first inner cell bg == CARD_BG
}

#[test]
fn header_bg_tints_toward_green_with_flash() {
    // .reload_flash(1.0) → inner cell bg != CARD_BG and != STATUS_GREEN (blended)
}
```

Inspect a header-area cell's `.bg` (or `.style().bg`) from the rendered
`TestTerminal`/`Buffer` to assert the color. Keep the assertion on a cell known to
carry the block background.

### Notes

- **One blend point.** Tinting the block bg in `render_main_header` covers both
  the single-session (title + shortcuts row) and multi-session (title + tabs)
  layouts, since both render the same outer `block`.
- **Reuse, don't re-derive.** Import `lerp_color` from `crate::widgets::shimmer`
  (or via the `widgets` re-export). Do not copy RGB interpolation into `header.rs`.
- **`RELOAD_FLASH_BLEND_CAP`** is a tuning constant — `0.35` is a reasonable
  subtle peak; adjust during the manual check if it reads too strong/weak, but
  keep it a named constant with a comment (no inline magic number).
- **Suppression is upstream.** Error/`Stopped`/`Quitting` suppression lives in
  task 01's helper, so the TUI does not branch on phase — it just consumes the
  alpha. Keep it that way.
- **No managed-doc change.** No `AppPhase`/`Message`/module-structure change, so
  `docs/ARCHITECTURE.md` needs no update for this phase.
