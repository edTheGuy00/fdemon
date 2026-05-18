## Task: Add `frame_hints` Core Helper and `FrameHint` Enum

**Objective**: Add a pure, table-driven helper in `fdemon-core` that derives a list of refresh-rate-aware diagnostic hints from a `FrameTiming` value. Mirrors DevTools' `frame_hints.dart` semantics so the Phase 2 Frame Analysis TUI tab can render hints without any rendering-layer derivation logic.

**Depends on**: None

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-core/src/frame_hints.rs` — **NEW** module containing the `FrameHint` enum, ordered constants, and the `frame_hints(frame, refresh_rate_hz) -> Vec<FrameHint>` builder. Inline unit tests via `#[cfg(test)] mod tests`.
- `crates/fdemon-core/src/lib.rs` — add `pub mod frame_hints;` re-export so consumers can `use fdemon_core::frame_hints::{frame_hints, FrameHint};`.
- `crates/fdemon-core/src/prelude.rs` — re-export `frame_hints::{frame_hints, FrameHint, FramePhaseKind}` for convenience (only if `prelude.rs` already re-exports `performance` types — check before editing).

**Files Read (Dependencies):**
- `crates/fdemon-core/src/performance.rs` — for `FrameTiming`, `FramePhases`, `FRAME_BUDGET_60FPS_MICROS` constants.
- `tmp/devtools/packages/devtools_app/lib/src/screens/performance/panes/frame_analysis/frame_hints.dart` — reference for hint copy + ordering.

### Details

#### `FrameHint` enum

```rust
/// Which phase of a frame consumed the most time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePhaseKind {
    Build,
    Layout,
    Paint,
    Raster,
}

/// Diagnostic hint derived from a single frame's timing.
///
/// Ordered by salience — the worst-news hint comes first. Renderers may
/// truncate to a fixed cap (Phase 2 uses 5). Use [`FrameHint::message`] to
/// retrieve the user-facing string; the enum carries no static text so
/// translation / theming can swap copy without touching the producer.
#[derive(Debug, Clone, PartialEq)]
pub enum FrameHint {
    /// Frame exceeded the per-frame budget for the current refresh rate.
    OverBudget { excess_ms: f64, budget_ms: f64 },
    /// Shader compilation was detected for this frame.
    ShaderCompilation,
    /// One UI phase consumed > 50% of the UI thread time.
    LongestUiPhase { phase: FramePhaseKind, share: f64 },
    /// Raster time exceeded UI time by > 50%.
    RasterDominant { ui_ms: f64, raster_ms: f64 },
    /// UI time exceeded raster time by > 50% (build-bound frame).
    BuildDominant { ui_ms: f64, raster_ms: f64 },
}

impl FrameHint {
    /// User-facing single-line summary (≤ 80 chars).
    pub fn message(&self) -> String { /* ... */ }
}
```

#### Builder

```rust
/// Derive up to 5 ordered diagnostic hints from a frame's timing.
///
/// Hints are ordered by salience:
/// 1. `OverBudget` (always first when present)
/// 2. `ShaderCompilation`
/// 3. `LongestUiPhase` (only when `phases` is `Some` and one phase > 50%)
/// 4. `RasterDominant` / `BuildDominant` (mutually exclusive)
///
/// `refresh_rate_hz` is used to compute the per-frame budget. The caller is
/// responsible for passing the runtime-detected refresh rate; Phase 2 will pass
/// the default `60.0` from `PerformanceState::display_refresh_rate`.
pub fn frame_hints(frame: &FrameTiming, refresh_rate_hz: f64) -> Vec<FrameHint> {
    // 1. compute budget_ms = 1000.0 / refresh_rate_hz
    // 2. if frame.elapsed_ms() > budget_ms → push OverBudget
    // 3. if frame.has_shader_compilation() → push ShaderCompilation
    // 4. if let Some(phases) = &frame.phases:
    //      compute share for each phase relative to phases.total_micros()
    //      push LongestUiPhase if max share > 0.5
    //    else:
    //      compare build_ms vs raster_ms; push BuildDominant or RasterDominant
    //      when one is > 1.5x the other
    // 5. truncate to MAX_HINTS_PER_FRAME (= 5)
}
```

#### Constants

```rust
/// Cap on hints per frame to keep the TUI tidy.
pub const MAX_HINTS_PER_FRAME: usize = 5;

/// Threshold above which a single UI phase is called out as dominant.
const LONGEST_PHASE_THRESHOLD: f64 = 0.5;

/// Threshold above which raster or build is called out as dominant.
const THREAD_DOMINANCE_RATIO: f64 = 1.5;
```

### Acceptance Criteria

1. `frame_hints(frame, 60.0)` returns `[]` for a non-janky frame with no shader compilation and balanced UI/raster (e.g. `build=4ms, raster=4ms, elapsed=8ms, phases=None`).
2. `frame_hints(frame, 60.0)` for a 24 ms frame returns `OverBudget { excess_ms ≈ 7.333, budget_ms ≈ 16.667 }` as the first entry.
3. `frame_hints(frame, 120.0)` for a 12 ms frame returns `OverBudget { excess_ms ≈ 3.667, budget_ms ≈ 8.333 }` — refresh-rate-aware.
4. `frame_hints(frame, 60.0)` for a frame with `shader_compilation=true` includes `ShaderCompilation` ordered after `OverBudget` (if both apply).
5. When `phases.is_some()` and one of `build / layout / paint` consumes > 50% of `ui_micros()`, `LongestUiPhase { phase: <kind>, share: > 0.5 }` is included.
6. When `phases.is_none()` and `raster_ms > 1.5 * build_ms`, `RasterDominant` is included; symmetrically for `BuildDominant`. The two are mutually exclusive — a balanced frame produces neither.
7. The returned `Vec` is at most `MAX_HINTS_PER_FRAME (= 5)` long.
8. `FrameHint::message()` returns a non-empty string ≤ 80 characters for every variant.
9. Module compiles standalone; no dependency on `fdemon-app` / `fdemon-tui`.

### Testing

Table-driven unit tests inline in `frame_hints.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance::{FramePhases, FrameTiming};

    fn frame(elapsed_us: u64, build_us: u64, raster_us: u64, phases: Option<FramePhases>) -> FrameTiming {
        FrameTiming {
            number: 1,
            build_micros: build_us,
            raster_micros: raster_us,
            elapsed_micros: elapsed_us,
            timestamp: chrono::Local::now(),
            phases,
            shader_compilation: false,
        }
    }

    #[test]
    fn no_hints_for_balanced_in_budget_frame() {
        let f = frame(10_000, 4_000, 4_000, None);
        assert!(frame_hints(&f, 60.0).is_empty());
    }

    #[test]
    fn over_budget_first_when_janky() {
        let f = frame(24_000, 10_000, 10_000, None);
        let hints = frame_hints(&f, 60.0);
        assert!(matches!(hints.first(), Some(FrameHint::OverBudget { .. })));
    }

    #[test]
    fn over_budget_respects_120hz_refresh_rate() {
        let f = frame(12_000, 6_000, 6_000, None);
        let hints = frame_hints(&f, 120.0);
        assert!(matches!(hints.first(), Some(FrameHint::OverBudget { budget_ms, .. }) if (budget_ms - 8.333).abs() < 0.01));
    }

    #[test]
    fn longest_ui_phase_when_phases_imbalanced() {
        let phases = FramePhases { build_micros: 8_000, layout_micros: 1_000, paint_micros: 1_000, raster_micros: 4_000, shader_compilation: false };
        let f = frame(14_000, 10_000, 4_000, Some(phases));
        let hints = frame_hints(&f, 60.0);
        assert!(hints.iter().any(|h| matches!(h, FrameHint::LongestUiPhase { phase: FramePhaseKind::Build, .. })));
    }

    #[test]
    fn raster_dominant_when_no_phases_and_raster_much_longer() {
        let f = frame(14_000, 2_000, 8_000, None);
        let hints = frame_hints(&f, 60.0);
        assert!(hints.iter().any(|h| matches!(h, FrameHint::RasterDominant { .. })));
        assert!(!hints.iter().any(|h| matches!(h, FrameHint::BuildDominant { .. })));
    }

    #[test]
    fn shader_compilation_hint_included() {
        let mut f = frame(20_000, 6_000, 6_000, None);
        f.shader_compilation = true;
        let hints = frame_hints(&f, 60.0);
        assert!(hints.iter().any(|h| matches!(h, FrameHint::ShaderCompilation)));
    }

    #[test]
    fn ordering_over_budget_before_shader() {
        let mut f = frame(24_000, 10_000, 10_000, None);
        f.shader_compilation = true;
        let hints = frame_hints(&f, 60.0);
        let over_budget_pos = hints.iter().position(|h| matches!(h, FrameHint::OverBudget { .. }));
        let shader_pos = hints.iter().position(|h| matches!(h, FrameHint::ShaderCompilation));
        assert!(over_budget_pos.unwrap() < shader_pos.unwrap());
    }

    #[test]
    fn hint_message_never_exceeds_80_chars() {
        for h in [
            FrameHint::OverBudget { excess_ms: 99.9, budget_ms: 16.667 },
            FrameHint::ShaderCompilation,
            FrameHint::LongestUiPhase { phase: FramePhaseKind::Build, share: 0.99 },
            FrameHint::RasterDominant { ui_ms: 4.0, raster_ms: 12.0 },
            FrameHint::BuildDominant { ui_ms: 12.0, raster_ms: 4.0 },
        ] {
            assert!(h.message().chars().count() <= 80, "{}", h.message());
        }
    }

    #[test]
    fn max_hints_per_frame_bound() {
        // Construct a frame that triggers every condition; verify ≤ 5 hints.
        let phases = FramePhases { build_micros: 8_000, layout_micros: 1_000, paint_micros: 1_000, raster_micros: 14_000, shader_compilation: true };
        let mut f = frame(24_000, 10_000, 14_000, Some(phases));
        f.shader_compilation = true;
        assert!(frame_hints(&f, 60.0).len() <= MAX_HINTS_PER_FRAME);
    }
}
```

### Notes

- **DO NOT** import `chrono` outside the test helper — the producer is timestamp-agnostic.
- **DO NOT** depend on `fdemon-app` or `fdemon-tui` from `fdemon-core` — this is a strict layer boundary.
- The hint text strings are owned by this module; renderers call `.message()` only. This keeps i18n / theming changes localized.
- The Dart `frame_hints.dart` reference covers slightly more cases than Phase 2 needs — defer "PlatformViews" / "JankSampling" hints to Phase 3.
- Add module-level docstring `//!` per `docs/CODE_STANDARDS.md` "Module Documentation".
- Public items (the enum, the function, the constants) need `///` doc comments per the same standard.

---

## Completion Summary

(Filled in by implementor after work completes.)
