## Task: Apply shimmer to the transient status label

**Objective**: Thread the global `animation_frame` into the bottom metadata bar and shimmer the phase **label** while the session is in a transient phase (`Initializing` / `Reloading` / `Quitting` / `is_busy`). Steady states (`Running` / `Stopped`) and the icon glyph keep their existing static rendering.

**Depends on**: 01-shimmer-helper

**Estimated Time**: 1–1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: add `animation_frame: u64` to `StatusInfo`; shimmer the label in `render_bottom_metadata`.
- `crates/fdemon-tui/src/render/mod.rs`: populate `animation_frame` when constructing `StatusInfo` (line ~201).
- `crates/fdemon-tui/src/widgets/log_view/tests.rs`: add `animation_frame` to existing `StatusInfo { .. }` literals; add shimmer assertions.

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/shimmer.rs`: `shimmer_phase`, `shimmer_spans` (from task 01).
- `crates/fdemon-tui/src/theme/styles.rs`: `phase_indicator` / `phase_indicator_busy` return `(icon, label, Style)`.
- `crates/fdemon-app/src/state.rs`: `AppState::animation_frame` field.

### Details

**1. Carry the frame into `StatusInfo`** (`log_view/mod.rs:37`):

```rust
pub struct StatusInfo<'a> {
    pub phase: &'a AppPhase,
    pub is_busy: bool,
    // ... existing fields ...
    /// Global animation frame, drives the shimmer on transient status labels.
    pub animation_frame: u64,
}
```

Populate it at the construction site in `render/mod.rs:201` (where `state` is in scope):

```rust
let status_info = widgets::StatusInfo {
    phase: &handle.session.phase,
    is_busy: handle.session.is_busy(),
    // ... existing fields ...
    animation_frame: state.animation_frame,
};
```

**2. Decide "transient" and shimmer the label** in `render_bottom_metadata` (`log_view/mod.rs:844`). The label `Span` is currently built at lines ~863–868:

```rust
let (icon, label, phase_style) = if status.is_busy {
    theme_styles::phase_indicator_busy(icons)
} else {
    theme_styles::phase_indicator(status.phase, icons)
};

// Transient = work in progress; steady = Running/Stopped.
let is_transient = status.is_busy
    || matches!(
        status.phase,
        AppPhase::Initializing | AppPhase::Reloading | AppPhase::Quitting
    );

let mut spans = vec![Span::raw(" "), Span::styled(icon, phase_style), Span::raw(" ")];
if is_transient {
    let phase = shimmer::shimmer_phase(status.animation_frame);
    let base = phase_style.fg.unwrap_or(palette::TEXT_SECONDARY);
    let highlight = palette::TEXT_BRIGHT;
    let modifier = phase_style.add_modifier; // preserve BOLD
    spans.extend(shimmer::shimmer_spans(label, base, highlight, phase, modifier));
} else {
    spans.push(Span::styled(label, phase_style));
}
```

- **Base color** = the phase's own fg (yellow for reloading, muted for starting, red for stopping) so the shimmer stays semantically colored; **highlight** = `TEXT_BRIGHT` for the bright head. Adjust highlight per phase only if review prefers a same-hue bright variant.
- **Preserve the modifier**: read `phase_style.add_modifier` and pass it through so BOLD survives.
- Only the **label** shimmers. The **icon glyph** keeps `phase_style` (a single sweeping char reads as noise).

**3. Keep steady states pixel-identical.** For `Running` / `Stopped`, the `else` branch reproduces the original `Span::styled(label, phase_style)` exactly — no behavioral change.

### Acceptance Criteria

1. `StatusInfo` gains `animation_frame: u64`; the construction site in `render/mod.rs` passes `state.animation_frame`.
2. While `phase ∈ {Initializing, Reloading, Quitting}` or `is_busy`, the label renders as multiple per-character spans whose fg varies across the label (shimmer present).
3. For `Running` and `Stopped` (not busy), the label renders as the original single static `Span::styled(label, phase_style)` — no shimmer, no extra spans.
4. The icon glyph is unchanged in all phases.
5. BOLD (and any other phase-style modifier) is preserved on the shimmered label.
6. All existing `StatusInfo { .. }` literals in `tests.rs` compile (each gets `animation_frame: <value>`).

### Testing

Add to `log_view/tests.rs` (mirror the existing metadata-bar test setup):

```rust
#[test]
fn reloading_label_shimmers_across_chars() {
    // is_busy or phase=Reloading, animation_frame set; render and assert the
    // label cells do NOT all share one fg (sweep produces a gradient).
}

#[test]
fn running_label_is_static_single_style() {
    // phase=Running, not busy; assert label cells share the steady STATUS_GREEN fg.
}

#[test]
fn shimmer_advances_with_animation_frame() {
    // Same status rendered at two different animation_frame values yields
    // different label fg distributions (sweep moved).
}
```

Update all existing `StatusInfo { .. }` constructions in `tests.rs` (11 sites: lines ~983, 1200, 1233, 1265, 1298, 1334, 1370, 1407, 2464, 2501, 2543) to include `animation_frame: 0` (or a meaningful value where the test asserts shimmer).

### Notes

- `AppPhase` is already imported in `log_view/mod.rs` (used by `StatusInfo.phase`); reuse it for the `matches!`.
- Compute `shimmer_phase` **once** here (single label), satisfying the PLAN's "compute frame once per render" guidance.
- If task 01 left `#[allow(dead_code)]` on the shimmer API, remove it now that it's consumed.
- No `docs/` updates: shimmer is purely visual with no new key/config surface. Leave the "animations off" toggle for PLAN Future Enhancements.
- Watch the 50-line function limit: `render_bottom_metadata` is already long (pre-existing). Keep the added transient-label logic tight; if it pushes the function materially larger, extract a small `fn status_label_spans(...) -> Vec<Span>` helper in the same file rather than inlining.
