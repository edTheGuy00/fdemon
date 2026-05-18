//! # Frame Diagnostic Hints
//!
//! Pure, table-driven helper that derives refresh-rate-aware diagnostic hints
//! from a [`FrameTiming`] value. Mirrors the semantics of DevTools'
//! `frame_hints.dart` so that the Frame Analysis TUI tab can render hints
//! without any derivation logic in the rendering layer.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use fdemon_core::frame_hints::{frame_hints, FrameHint};
//!
//! let hints = frame_hints(&timing, 60.0);
//! for hint in &hints {
//!     println!("{}", hint.message());
//! }
//! ```

use crate::performance::FrameTiming;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Cap on hints per frame to keep the TUI tidy.
pub const MAX_HINTS_PER_FRAME: usize = 5;

/// Threshold above which a single UI phase is called out as dominant.
const LONGEST_PHASE_THRESHOLD: f64 = 0.5;

/// Threshold above which raster or build is called out as dominant.
const THREAD_DOMINANCE_RATIO: f64 = 1.5;

// ── FramePhaseKind ────────────────────────────────────────────────────────────

/// Which phase of a frame consumed the most time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePhaseKind {
    /// Widget tree construction phase.
    Build,
    /// Layout computation phase.
    Layout,
    /// Painting / compositing phase.
    Paint,
    /// GPU rasterization phase.
    Raster,
}

impl FramePhaseKind {
    /// Human-readable phase name for display.
    pub fn display_name(self) -> &'static str {
        match self {
            FramePhaseKind::Build => "Build",
            FramePhaseKind::Layout => "Layout",
            FramePhaseKind::Paint => "Paint",
            FramePhaseKind::Raster => "Raster",
        }
    }
}

// ── FrameHint ─────────────────────────────────────────────────────────────────

/// Diagnostic hint derived from a single frame's timing.
///
/// Ordered by salience — the worst-news hint comes first. Renderers may
/// truncate to a fixed cap (Phase 2 uses [`MAX_HINTS_PER_FRAME`] = 5). Use
/// [`FrameHint::message`] to retrieve the user-facing string; the enum carries
/// no static text so copy can be updated without touching the producer.
#[derive(Debug, Clone, PartialEq)]
pub enum FrameHint {
    /// Frame exceeded the per-frame budget for the current refresh rate.
    OverBudget {
        /// Time over the budget in milliseconds.
        excess_ms: f64,
        /// The per-frame budget in milliseconds for the given refresh rate.
        budget_ms: f64,
    },
    /// Shader compilation was detected for this frame.
    ShaderCompilation,
    /// One UI phase consumed > 50% of the UI thread time.
    LongestUiPhase {
        /// The dominant phase.
        phase: FramePhaseKind,
        /// Fraction of total UI thread time consumed by this phase (0.0–1.0).
        share: f64,
    },
    /// Raster time exceeded UI time by > 50% — GPU-bound frame.
    RasterDominant {
        /// UI thread time in milliseconds.
        ui_ms: f64,
        /// Raster thread time in milliseconds.
        raster_ms: f64,
    },
    /// UI time exceeded raster time by > 50% — build/CPU-bound frame.
    BuildDominant {
        /// UI thread time in milliseconds.
        ui_ms: f64,
        /// Raster thread time in milliseconds.
        raster_ms: f64,
    },
}

impl FrameHint {
    /// User-facing single-line summary (≤ 80 chars).
    pub fn message(&self) -> String {
        match self {
            FrameHint::OverBudget {
                excess_ms,
                budget_ms,
            } => {
                format!(
                    "Over budget by {:.1}ms (budget: {:.1}ms)",
                    excess_ms, budget_ms
                )
            }
            FrameHint::ShaderCompilation => {
                "Shader compilation detected — may cause jank on first run".to_string()
            }
            FrameHint::LongestUiPhase { phase, share } => {
                format!(
                    "{} consumed {:.0}% of UI thread time",
                    phase.display_name(),
                    share * 100.0
                )
            }
            FrameHint::RasterDominant { ui_ms, raster_ms } => {
                format!(
                    "Raster-bound: {:.1}ms raster vs {:.1}ms UI",
                    raster_ms, ui_ms
                )
            }
            FrameHint::BuildDominant { ui_ms, raster_ms } => {
                format!("UI-bound: {:.1}ms UI vs {:.1}ms raster", ui_ms, raster_ms)
            }
        }
    }
}

// ── Builder ───────────────────────────────────────────────────────────────────

/// Derive up to [`MAX_HINTS_PER_FRAME`] ordered diagnostic hints from a
/// frame's timing data.
///
/// Hints are ordered by salience:
/// 1. [`FrameHint::OverBudget`] — always first when the frame is over budget
/// 2. [`FrameHint::ShaderCompilation`] — when shader compilation is detected
/// 3. [`FrameHint::LongestUiPhase`] — only when `phases` is `Some` and one
///    phase consumes > 50% of UI thread time
/// 4. [`FrameHint::RasterDominant`] / [`FrameHint::BuildDominant`] — mutually
///    exclusive; emitted when one thread time is > 1.5× the other
///
/// `refresh_rate_hz` is used to compute the per-frame budget. The caller is
/// responsible for passing the runtime-detected refresh rate; pass `60.0` as a
/// sensible default.
pub fn frame_hints(frame: &FrameTiming, refresh_rate_hz: f64) -> Vec<FrameHint> {
    let mut hints: Vec<FrameHint> = Vec::new();

    // 1. Compute per-frame budget from refresh rate.
    let budget_ms = 1000.0 / refresh_rate_hz;
    let elapsed_ms = frame.elapsed_ms();

    // 2. Over-budget hint — always first.
    if elapsed_ms > budget_ms {
        let excess_ms = elapsed_ms - budget_ms;
        hints.push(FrameHint::OverBudget {
            excess_ms,
            budget_ms,
        });
    }

    // 3. Shader compilation hint.
    if frame.has_shader_compilation() {
        hints.push(FrameHint::ShaderCompilation);
    }

    // 4a. Phase-level breakdown when phases are available.
    if let Some(phases) = &frame.phases {
        let ui_total = phases.ui_micros();
        if ui_total > 0 {
            // Find the dominant UI phase (build / layout / paint; not raster).
            let candidates = [
                (FramePhaseKind::Build, phases.build_micros),
                (FramePhaseKind::Layout, phases.layout_micros),
                (FramePhaseKind::Paint, phases.paint_micros),
            ];
            if let Some((kind, micros)) = candidates.iter().copied().max_by_key(|&(_, m)| m) {
                let share = micros as f64 / ui_total as f64;
                if share > LONGEST_PHASE_THRESHOLD {
                    hints.push(FrameHint::LongestUiPhase { phase: kind, share });
                }
            }
        }
    } else {
        // 4b. Thread-level comparison when phase breakdown is unavailable.
        let build_ms = frame.build_ms();
        let raster_ms = frame.raster_ms();

        if raster_ms > THREAD_DOMINANCE_RATIO * build_ms {
            hints.push(FrameHint::RasterDominant {
                ui_ms: build_ms,
                raster_ms,
            });
        } else if build_ms > THREAD_DOMINANCE_RATIO * raster_ms {
            hints.push(FrameHint::BuildDominant {
                ui_ms: build_ms,
                raster_ms,
            });
        }
    }

    // 5. Enforce maximum hint count.
    hints.truncate(MAX_HINTS_PER_FRAME);
    hints
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance::{FramePhases, FrameTiming};

    fn frame(
        elapsed_us: u64,
        build_us: u64,
        raster_us: u64,
        phases: Option<FramePhases>,
    ) -> FrameTiming {
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
        assert!(matches!(
            hints.first(),
            Some(FrameHint::OverBudget { budget_ms, .. }) if (budget_ms - 8.333).abs() < 0.01
        ));
    }

    #[test]
    fn longest_ui_phase_when_phases_imbalanced() {
        let phases = FramePhases {
            build_micros: 8_000,
            layout_micros: 1_000,
            paint_micros: 1_000,
            raster_micros: 4_000,
            shader_compilation: false,
        };
        let f = frame(14_000, 10_000, 4_000, Some(phases));
        let hints = frame_hints(&f, 60.0);
        assert!(hints.iter().any(|h| matches!(
            h,
            FrameHint::LongestUiPhase {
                phase: FramePhaseKind::Build,
                ..
            }
        )));
    }

    #[test]
    fn raster_dominant_when_no_phases_and_raster_much_longer() {
        let f = frame(14_000, 2_000, 8_000, None);
        let hints = frame_hints(&f, 60.0);
        assert!(hints
            .iter()
            .any(|h| matches!(h, FrameHint::RasterDominant { .. })));
        assert!(!hints
            .iter()
            .any(|h| matches!(h, FrameHint::BuildDominant { .. })));
    }

    #[test]
    fn shader_compilation_hint_included() {
        let mut f = frame(20_000, 6_000, 6_000, None);
        f.shader_compilation = true;
        let hints = frame_hints(&f, 60.0);
        assert!(hints
            .iter()
            .any(|h| matches!(h, FrameHint::ShaderCompilation)));
    }

    #[test]
    fn ordering_over_budget_before_shader() {
        let mut f = frame(24_000, 10_000, 10_000, None);
        f.shader_compilation = true;
        let hints = frame_hints(&f, 60.0);
        let over_budget_pos = hints
            .iter()
            .position(|h| matches!(h, FrameHint::OverBudget { .. }));
        let shader_pos = hints
            .iter()
            .position(|h| matches!(h, FrameHint::ShaderCompilation));
        assert!(over_budget_pos.unwrap() < shader_pos.unwrap());
    }

    #[test]
    fn hint_message_never_exceeds_80_chars() {
        for h in [
            FrameHint::OverBudget {
                excess_ms: 99.9,
                budget_ms: 16.667,
            },
            FrameHint::ShaderCompilation,
            FrameHint::LongestUiPhase {
                phase: FramePhaseKind::Build,
                share: 0.99,
            },
            FrameHint::RasterDominant {
                ui_ms: 4.0,
                raster_ms: 12.0,
            },
            FrameHint::BuildDominant {
                ui_ms: 12.0,
                raster_ms: 4.0,
            },
        ] {
            assert!(h.message().chars().count() <= 80, "{}", h.message());
        }
    }

    #[test]
    fn max_hints_per_frame_bound() {
        // Construct a frame that triggers every condition; verify ≤ 5 hints.
        let phases = FramePhases {
            build_micros: 8_000,
            layout_micros: 1_000,
            paint_micros: 1_000,
            raster_micros: 14_000,
            shader_compilation: true,
        };
        let mut f = frame(24_000, 10_000, 14_000, Some(phases));
        f.shader_compilation = true;
        assert!(frame_hints(&f, 60.0).len() <= MAX_HINTS_PER_FRAME);
    }

    #[test]
    fn build_dominant_when_no_phases_and_build_much_longer() {
        let f = frame(14_000, 8_000, 2_000, None);
        let hints = frame_hints(&f, 60.0);
        assert!(hints
            .iter()
            .any(|h| matches!(h, FrameHint::BuildDominant { .. })));
        assert!(!hints
            .iter()
            .any(|h| matches!(h, FrameHint::RasterDominant { .. })));
    }

    #[test]
    fn balanced_frame_no_dominant_thread_hint() {
        // 6ms build, 8ms raster — ratio is 1.33, below THREAD_DOMINANCE_RATIO of 1.5
        let f = frame(14_000, 6_000, 8_000, None);
        let hints = frame_hints(&f, 60.0);
        assert!(!hints
            .iter()
            .any(|h| matches!(h, FrameHint::RasterDominant { .. })));
        assert!(!hints
            .iter()
            .any(|h| matches!(h, FrameHint::BuildDominant { .. })));
    }

    #[test]
    fn over_budget_values_are_correct() {
        // 24ms elapsed at 60hz budget of ~16.667ms → excess ~7.333ms
        let f = frame(24_000, 10_000, 10_000, None);
        let hints = frame_hints(&f, 60.0);
        if let Some(FrameHint::OverBudget {
            excess_ms,
            budget_ms,
        }) = hints.first()
        {
            assert!((budget_ms - 16.667).abs() < 0.01, "budget_ms={}", budget_ms);
            assert!((excess_ms - 7.333).abs() < 0.01, "excess_ms={}", excess_ms);
        } else {
            panic!("Expected OverBudget as first hint");
        }
    }

    #[test]
    fn longest_ui_phase_share_is_accurate() {
        // build=8000, layout=1000, paint=1000 → ui_total=10000 → build share=0.8
        let phases = FramePhases {
            build_micros: 8_000,
            layout_micros: 1_000,
            paint_micros: 1_000,
            raster_micros: 4_000,
            shader_compilation: false,
        };
        let f = frame(14_000, 10_000, 4_000, Some(phases));
        let hints = frame_hints(&f, 60.0);
        let found = hints.iter().find_map(|h| {
            if let FrameHint::LongestUiPhase { phase, share } = h {
                Some((*phase, *share))
            } else {
                None
            }
        });
        let (kind, share) = found.expect("Expected LongestUiPhase hint");
        assert_eq!(kind, FramePhaseKind::Build);
        assert!((share - 0.8).abs() < 0.001, "share={}", share);
    }

    #[test]
    fn layout_dominant_phase_is_detected() {
        // layout consumes 70% of UI time
        let phases = FramePhases {
            build_micros: 1_000,
            layout_micros: 7_000,
            paint_micros: 2_000,
            raster_micros: 3_000,
            shader_compilation: false,
        };
        let f = frame(13_000, 10_000, 3_000, Some(phases));
        let hints = frame_hints(&f, 60.0);
        assert!(hints.iter().any(|h| matches!(
            h,
            FrameHint::LongestUiPhase {
                phase: FramePhaseKind::Layout,
                ..
            }
        )));
    }

    #[test]
    fn no_longest_phase_when_evenly_split() {
        // Each phase = 33% → none exceeds 50%
        let phases = FramePhases {
            build_micros: 3_000,
            layout_micros: 3_000,
            paint_micros: 3_000,
            raster_micros: 3_000,
            shader_compilation: false,
        };
        let f = frame(12_000, 9_000, 3_000, Some(phases));
        let hints = frame_hints(&f, 60.0);
        assert!(!hints
            .iter()
            .any(|h| matches!(h, FrameHint::LongestUiPhase { .. })));
    }
}
