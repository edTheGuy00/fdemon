## Task: Shimmer helper module (`widgets/shimmer.rs`)

**Objective**: Add a pure, reusable shimmer helper — RGB color interpolation, a frame-driven phase, and a per-character span builder that produces a left-to-right color sweep. No consumers yet; this is the foundation task 02 (and Phase 6) build on.

**Depends on**: None

**Estimated Time**: 1–1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/shimmer.rs` (new): the helper + inline `#[cfg(test)] mod tests`.
- `crates/fdemon-tui/src/widgets/mod.rs`: register the module and re-export its public API.

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/theme/palette.rs`: color constants are `Color::Rgb`; use as reference for callers (the helper itself takes RGB tuples / `Color`).

### Details

Create `widgets/shimmer.rs` with a `//!` module header describing the sweep effect. Keep every function under 50 lines and give each `pub` item a `///` doc comment.

**1. RGB lerp** — works on `ratatui::style::Color`, degrades gracefully for non-RGB:

```rust
/// Linearly interpolate between two colors. `t` is clamped to `[0.0, 1.0]`.
/// If either color is not `Color::Rgb`, returns `a` unchanged (graceful
/// fallback for 16/256-color terminals, which crossterm down-converts anyway).
pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => {
            let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
            Color::Rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
        }
        _ => a,
    }
}
```

**2. Shimmer phase** — a normalized 0→1 sweep position derived from the global frame:

```rust
/// Frames per full shimmer sweep (~1.5 s at the 50 ms / 20 fps tick cadence).
const SHIMMER_PERIOD_FRAMES: u64 = 30;

/// Current sweep position in `[0.0, 1.0)`, derived from the global animation
/// frame. Wraps cleanly via modulo so `u64` wrap in the source frame is fine.
pub fn shimmer_phase(frame: u64) -> f32 {
    (frame % SHIMMER_PERIOD_FRAMES) as f32 / SHIMMER_PERIOD_FRAMES as f32
}
```

**3. Span builder** — tint each character between `base` and `highlight` by distance from a moving "head," preserving caller modifiers:

```rust
/// Width of the bright "head" of the sweep, in characters.
const SHIMMER_HEAD_WIDTH: f32 = 4.0;

/// Build shimmered spans for `text`: each character's fg is lerped between
/// `base` and `highlight` based on its distance from a head that sweeps left
/// to right as `phase` advances. `modifier` (e.g. BOLD) is applied to every
/// span so the caller's emphasis is preserved. Empty `text` yields no spans.
pub fn shimmer_spans(
    text: &str,
    base: Color,
    highlight: Color,
    phase: f32,
    modifier: Modifier,
) -> Vec<Span<'static>> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }
    let head = phase * chars.len() as f32;
    chars
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let dist = (i as f32 - head).abs();
            let t = (1.0 - dist / SHIMMER_HEAD_WIDTH).max(0.0); // 1 at head → 0 away
            let fg = lerp_color(base, highlight, t);
            Span::styled(c.to_string(), Style::default().fg(fg).add_modifier(modifier))
        })
        .collect()
}
```

Register in `widgets/mod.rs` alongside the existing helper modules (model after `tag_filter`):

```rust
pub mod shimmer;
// ...
pub use shimmer::{lerp_color, shimmer_phase, shimmer_spans};
```

### Acceptance Criteria

1. `lerp_color(a, b, 0.0) == a`, `lerp_color(a, b, 1.0) == b` for two `Color::Rgb` inputs; midpoint rounds component-wise.
2. `lerp_color` clamps `t` outside `[0,1]` and returns the base color unchanged when either input is non-RGB.
3. `shimmer_phase` returns a value in `[0.0, 1.0)` and wraps (frame `0` and frame `SHIMMER_PERIOD_FRAMES` give the same phase); does not panic near `u64::MAX`.
4. `shimmer_spans` returns one span per character, empty `Vec` for empty text, and applies the given `Modifier` to every span.
5. The character nearest the head is closest to `highlight`; characters beyond `SHIMMER_HEAD_WIDTH` of the head equal `base`.
6. Public API (`lerp_color`, `shimmer_phase`, `shimmer_spans`) is re-exported from `widgets/mod.rs`.

### Testing

```rust
#[test]
fn lerp_endpoints_and_midpoint() { /* 0.0→a, 1.0→b, 0.5→component midpoint */ }

#[test]
fn lerp_non_rgb_falls_back_to_base() { /* Color::Yellow input returns base */ }

#[test]
fn shimmer_phase_wraps_over_period() { /* phase(0) == phase(SHIMMER_PERIOD_FRAMES) */ }

#[test]
fn shimmer_spans_one_per_char_and_empty() { /* len matches; "" → [] */ }

#[test]
fn shimmer_spans_head_is_brightest() { /* span nearest head closer to highlight than far span */ }

#[test]
fn shimmer_spans_preserves_modifier() { /* every span carries BOLD when requested */ }
```

### Notes

- Keep this module **pure**: no `AppState`, no rendering side effects, no I/O — only color math and span construction. This keeps it trivially testable and reusable by Phase 6.
- `SHIMMER_PERIOD_FRAMES` and `SHIMMER_HEAD_WIDTH` are named constants with derivation doc comments (no magic numbers, per CODE_STANDARDS Principle 4).
- Returning `Vec<Span<'static>>` (owned `String` content) avoids borrowing-lifetime friction at the call site; the label text is short, so allocation cost is negligible.
- Do not wire any caller here — task 02 owns the integration. This task must build and pass tests on its own (the new `pub` items will be unused until task 02; add `#[allow(dead_code)]` only if clippy flags them, and remove it in task 02).

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/shimmer.rs` | New file — `lerp_color`, `shimmer_phase`, `shimmer_spans`, named constants, 9 inline unit tests |
| `crates/fdemon-tui/src/widgets/mod.rs` | Added `pub mod shimmer;` and re-export of `lerp_color`, `shimmer_phase`, `shimmer_spans` |

### Notable Decisions/Tradeoffs

1. **Clippy manual_range_contains**: The range check in the shimmer_phase test was written as `p >= 0.0 && p < 1.0` then updated to `(0.0..1.0).contains(&p)` to satisfy `-D warnings` with the `manual_range_contains` lint.
2. **Extra tests beyond spec**: Added `lerp_clamps_t_outside_range`, `shimmer_phase_no_panic_near_u64_max`, and `shimmer_spans_unicode_multibyte` beyond the 6 required tests. These exercise the acceptance criteria edge cases more thoroughly and added no complexity.
3. **No `#[allow(dead_code)]` needed**: Clippy did not flag the re-exported public items as dead code — the `pub use` in `widgets/mod.rs` is sufficient to silence the lint.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test -p fdemon-tui` — Passed (1310 unit tests + 7 doc-tests)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **No callers yet**: `lerp_color`, `shimmer_phase`, and `shimmer_spans` are unused until task 02 wires them into the status bar; this is expected per task design.
