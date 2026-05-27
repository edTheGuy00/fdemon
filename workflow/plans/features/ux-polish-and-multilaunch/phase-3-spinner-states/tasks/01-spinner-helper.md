## Task: Spinner helper module (`widgets/spinner.rs`)

**Objective**: Add a pure, reusable braille-spinner helper — the frame set plus a frame-driven `spinner_char(frame: u64) -> char` — mirroring the Phase 2 `widgets/shimmer.rs` model. No consumers yet; this is the foundation tasks 02 and 03 build on.

**Depends on**: None

**Estimated Time**: 0.5–1h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/spinner.rs` (new): the helper + inline `#[cfg(test)] mod tests`.
- `crates/fdemon-tui/src/widgets/mod.rs`: register the module and re-export its public API (model after the existing `shimmer` registration on lines 12 and 24).

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/shimmer.rs`: pattern reference (pure module, named constants with derivation comments, inline tests).

### Details

Create `widgets/spinner.rs` with a `//!` module header describing the throbber. Keep it **pure**: no `AppState`, no rendering, no I/O — only the frame set and index math. Give each `pub` item a `///` doc comment. No magic numbers (CODE_STANDARDS Principle 4).

```rust
//! # Spinner Helper
//!
//! Pure frame-selection math for a braille throbber. Callers supply a
//! monotonically advancing `u64` frame (e.g. `AppState::animation_frame`) and
//! receive the glyph to draw this frame. Intentionally has no `AppState`,
//! rendering, or I/O dependency so it is trivially testable and reusable.

/// Braille throbber frames, in sweep order. Lifted verbatim from the inline
/// `SPINNER` constant previously in `render_loading_screen` so the startup
/// screen has zero visual change after task 02 adopts this helper.
pub const SPINNER_FRAMES: &[char] =
    &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Ticks per spinner frame for callers driving the spinner off the global
/// `AppState::animation_frame` (which advances every ~50 ms / 20 fps tick).
/// At 2 ticks/frame the throbber advances ~every 100 ms (~10 fps) — lively but
/// calm. The loading screen does NOT use this divisor (see `spinner_char`).
pub const SPINNER_TICKS_PER_FRAME: u64 = 2;

/// The throbber glyph for the given frame index, via direct modulo over
/// `SPINNER_FRAMES`. `frame` is the already-cadence-adjusted index: the caller
/// decides how fast to advance (pass the raw frame for one-glyph-per-tick, or
/// `frame / SPINNER_TICKS_PER_FRAME` for the calmer dialog cadence). Wraps
/// cleanly via `%`, so `u64` wrap in the source frame is harmless.
pub fn spinner_char(frame: u64) -> char {
    SPINNER_FRAMES[(frame % SPINNER_FRAMES.len() as u64) as usize]
}
```

Register in `widgets/mod.rs` next to the `shimmer` lines:

```rust
pub mod spinner;
// ...
pub use spinner::{spinner_char, SPINNER_FRAMES, SPINNER_TICKS_PER_FRAME};
```

### Acceptance Criteria

1. `spinner_char(0) == SPINNER_FRAMES[0]` (`'⠋'`), and `spinner_char(n)` advances deterministically: `spinner_char(i) == SPINNER_FRAMES[i % len]` for a range of `i`.
2. `spinner_char` wraps over the frame set: `spinner_char(SPINNER_FRAMES.len() as u64) == spinner_char(0)`.
3. `spinner_char(u64::MAX)` does not panic.
4. `SPINNER_FRAMES` is exactly the 10 glyphs `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` in that order (must match the constant currently in `render/mod.rs:523`).
5. Public API (`spinner_char`, `SPINNER_FRAMES`, `SPINNER_TICKS_PER_FRAME`) is re-exported from `widgets/mod.rs`.

### Testing

```rust
#[test]
fn spinner_char_advances_deterministically() { /* i % len for i in 0..25 */ }

#[test]
fn spinner_char_wraps_over_frame_set() { /* char(len) == char(0) */ }

#[test]
fn spinner_char_no_panic_near_u64_max() { let _ = spinner_char(u64::MAX); }

#[test]
fn spinner_frames_match_legacy_constant() { /* assert the 10 glyphs, in order */ }
```

### Notes

- Keep this module pure and dependency-free, exactly like `shimmer.rs`. This is what makes the "advances deterministically per frame (unit-tested)" success criterion trivial.
- The new `pub` items are unused until tasks 02/03 wire them. If clippy flags dead code, add a temporary `#[allow(dead_code)]` and remove it in task 02 — but the `pub use` re-export usually suffices (it did for `shimmer.rs`).
- Do **not** edit `render/mod.rs` or any dialog widget here — those are tasks 02 and 03. This task must build and pass tests on its own.
