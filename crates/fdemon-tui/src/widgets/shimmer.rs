//! # Shimmer Helper
//!
//! Pure color-math utilities for a left-to-right shimmer sweep effect.
//!
//! A "shimmer" is a bright "head" that sweeps across text from left to right,
//! lerping each character's foreground color between a `base` dim color and a
//! `highlight` bright color based on the character's distance from the head.
//!
//! This module is intentionally **pure**: no `AppState`, no rendering side
//! effects, no I/O — only color math and span construction. This makes it
//! trivially testable and reusable by multiple callers.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

/// Frames per full shimmer sweep (~1.5 s at the 50 ms / 20 fps tick cadence).
const SHIMMER_PERIOD_FRAMES: u64 = 30;

/// Width of the bright "head" of the sweep, in characters.
const SHIMMER_HEAD_WIDTH: f32 = 3.5;

/// How far (in characters) the sweep head travels off-screen past each edge,
/// so it fades in from the left, exits off the right, and leaves a brief
/// all-dim rest gap between cycles instead of popping in / snapping back.
const SHIMMER_LEAD: f32 = 3.0;

/// Linearly interpolate between two colors. `t` is clamped to `[0.0, 1.0]`.
///
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

/// Current sweep position in `[0.0, 1.0)`, derived from the global animation
/// frame. Wraps cleanly via modulo so `u64` wrap in the source frame is fine.
pub fn shimmer_phase(frame: u64) -> f32 {
    (frame % SHIMMER_PERIOD_FRAMES) as f32 / SHIMMER_PERIOD_FRAMES as f32
}

/// Build shimmered spans for `text`: each character's fg is lerped between
/// `base` and `highlight` based on its distance from a head that sweeps left
/// to right as `phase` advances. `modifier` (e.g. `BOLD`) is applied to every
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
    // Map the head position into [-SHIMMER_LEAD, n + SHIMMER_LEAD) so it
    // travels off-screen on both sides.  This produces a soft fade-in from the
    // left edge, a soft fade-out off the right edge, and a brief all-dim rest
    // gap between cycles — instead of snapping back to character 0.
    let n = chars.len() as f32;
    let head = phase * (n + SHIMMER_LEAD * 2.0) - SHIMMER_LEAD;
    chars
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let dist = (i as f32 - head).abs();
            let t = (1.0 - dist / SHIMMER_HEAD_WIDTH).max(0.0); // 1 at head → 0 away
            let fg = lerp_color(base, highlight, t);
            Span::styled(
                c.to_string(),
                Style::default().fg(fg).add_modifier(modifier),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    #[test]
    fn lerp_endpoints_and_midpoint() {
        let a = Color::Rgb(0, 0, 0);
        let b = Color::Rgb(200, 100, 50);

        // t=0.0 → a
        assert_eq!(lerp_color(a, b, 0.0), a);

        // t=1.0 → b
        assert_eq!(lerp_color(a, b, 1.0), b);

        // t=0.5 → component-wise midpoint (rounded)
        let mid = lerp_color(a, b, 0.5);
        assert_eq!(mid, Color::Rgb(100, 50, 25));
    }

    #[test]
    fn lerp_non_rgb_falls_back_to_base() {
        let a = Color::Yellow;
        let b = Color::Rgb(88, 166, 255);

        // When `a` is non-RGB, returns `a` unchanged regardless of t.
        assert_eq!(lerp_color(a, b, 0.5), a);
        assert_eq!(lerp_color(a, b, 1.0), a);

        // When `b` is non-RGB but `a` is RGB, also falls back to `a`.
        let a2 = Color::Rgb(88, 166, 255);
        let b2 = Color::Green;
        assert_eq!(lerp_color(a2, b2, 0.5), a2);
    }

    #[test]
    fn lerp_clamps_t_outside_range() {
        let a = Color::Rgb(0, 0, 0);
        let b = Color::Rgb(100, 100, 100);

        // t < 0.0 clamped to 0.0 → a
        assert_eq!(lerp_color(a, b, -1.0), a);

        // t > 1.0 clamped to 1.0 → b
        assert_eq!(lerp_color(a, b, 2.0), b);
    }

    #[test]
    fn shimmer_phase_wraps_over_period() {
        // phase(0) and phase(SHIMMER_PERIOD_FRAMES) must be identical (both 0.0)
        let phase_at_zero = shimmer_phase(0);
        let phase_at_period = shimmer_phase(SHIMMER_PERIOD_FRAMES);
        assert_eq!(phase_at_zero, phase_at_period);
        assert_eq!(phase_at_zero, 0.0);

        // Phase must be in [0.0, 1.0)
        for frame in 0..SHIMMER_PERIOD_FRAMES {
            let p = shimmer_phase(frame);
            assert!(
                (0.0..1.0).contains(&p),
                "phase {p} out of [0, 1) for frame {frame}"
            );
        }
    }

    #[test]
    fn shimmer_phase_no_panic_near_u64_max() {
        // Must not panic near u64::MAX (wraps via %)
        let _ = shimmer_phase(u64::MAX);
        let _ = shimmer_phase(u64::MAX - 1);
    }

    #[test]
    fn shimmer_spans_one_per_char_and_empty() {
        // Empty text → empty Vec
        let spans = shimmer_spans(
            "",
            Color::Rgb(0, 0, 0),
            Color::Rgb(255, 255, 255),
            0.0,
            Modifier::empty(),
        );
        assert!(spans.is_empty());

        // Non-empty text → one span per character
        let text = "Hello";
        let spans = shimmer_spans(
            text,
            Color::Rgb(0, 0, 0),
            Color::Rgb(255, 255, 255),
            0.0,
            Modifier::empty(),
        );
        assert_eq!(spans.len(), text.chars().count());

        // Verify each span's content is a single character matching the input
        for (span, ch) in spans.iter().zip(text.chars()) {
            assert_eq!(span.content.as_ref(), ch.to_string().as_str());
        }
    }

    #[test]
    fn shimmer_spans_head_is_brightest() {
        let base = Color::Rgb(50, 50, 50);
        let highlight = Color::Rgb(250, 250, 250);
        let text = "ABCDEFGHIJ"; // 10 chars, n=10

        // Choose a phase that places the head over interior index 4.
        // head = phase * (n + SHIMMER_LEAD*2) - SHIMMER_LEAD
        //      = phase * 16 - 3 = 4  →  phase = 7/16 = 0.4375
        let phase = 7.0_f32 / 16.0;
        let spans = shimmer_spans(text, base, highlight, phase, Modifier::empty());

        // Character 4 is nearest the head and should be brightest.
        let fg_at_head = match spans[4].style.fg {
            Some(Color::Rgb(r, _, _)) => r,
            _ => panic!("expected Rgb color at index 4"),
        };

        // Character 0: dist = |0 - 4| = 4 > SHIMMER_HEAD_WIDTH(3.5) → base
        let fg_far_left = spans[0].style.fg;
        assert_eq!(
            fg_far_left,
            Some(base),
            "char at dist >3.5 from head (index 0) should equal base"
        );

        // Character 9: dist = |9 - 4| = 5 > SHIMMER_HEAD_WIDTH(3.5) → base
        let fg_far_right = spans[9].style.fg;
        assert_eq!(
            fg_far_right,
            Some(base),
            "char at dist >3.5 from head (index 9) should equal base"
        );

        // Index 4 must be brighter than both far ends.
        let base_r = match base {
            Color::Rgb(r, _, _) => r,
            _ => panic!("expected Rgb"),
        };
        assert!(
            fg_at_head > base_r,
            "head char r={fg_at_head} should be brighter than base r={base_r}"
        );
    }

    /// At `phase = 0.0` the head is at `-SHIMMER_LEAD = -3.0`.  Index 0 is
    /// `3.0` characters away → `t ≈ 0.143` (dim, not full highlight).  This
    /// proves there is no pop-in at cycle start.
    #[test]
    fn shimmer_spans_no_pop_in_at_phase_zero() {
        let base = Color::Rgb(50, 50, 50);
        let highlight = Color::Rgb(250, 250, 250);
        let text = "ABCDEFGHIJ"; // 10 chars

        let spans = shimmer_spans(text, base, highlight, 0.0, Modifier::empty());

        // At phase=0.0, head = -3.0.  Index 0 is only 3/3.5 of the way from
        // the head to off-screen → dim, not full highlight.
        let fg_0 = match spans[0].style.fg {
            Some(Color::Rgb(r, _, _)) => r,
            _ => panic!("expected Rgb at index 0"),
        };
        let base_r = match base {
            Color::Rgb(r, _, _) => r,
            _ => panic!("expected Rgb base"),
        };
        let highlight_r = match highlight {
            Color::Rgb(r, _, _) => r,
            _ => panic!("expected Rgb highlight"),
        };
        let midpoint_r = (base_r as u16 + highlight_r as u16) / 2;

        // Index 0 should be closer to base than to highlight (no pop-in).
        assert!(
            fg_0 < midpoint_r as u8,
            "at phase=0.0, index 0 r={fg_0} should be closer to base ({base_r}) than highlight ({highlight_r})"
        );
    }

    /// There exists a phase range where the head is fully off-screen to the
    /// right and every character is at `base` (the rest gap).
    ///
    /// For n=10, SHIMMER_LEAD=3.0, SHIMMER_HEAD_WIDTH=3.5:
    ///   head > (n-1) + SHIMMER_HEAD_WIDTH  →  head > 12.5
    ///   phase * 16 - 3 > 12.5  →  phase > 15.5/16 ≈ 0.96875
    ///
    /// At phase=0.97 the head is at 12.52, 3.52 past the last char → t=0 for all.
    #[test]
    fn shimmer_spans_rest_gap_all_at_base() {
        let base = Color::Rgb(50, 50, 50);
        let highlight = Color::Rgb(250, 250, 250);
        let text = "ABCDEFGHIJ"; // 10 chars

        let spans = shimmer_spans(text, base, highlight, 0.97, Modifier::empty());

        // Every character must equal base (head is fully off-screen right).
        for (i, span) in spans.iter().enumerate() {
            assert_eq!(
                span.style.fg,
                Some(base),
                "at phase=0.97, index {i} should be at base (rest gap)"
            );
        }
    }

    #[test]
    fn shimmer_spans_preserves_modifier() {
        let spans = shimmer_spans(
            "Bold",
            Color::Rgb(50, 50, 50),
            Color::Rgb(200, 200, 200),
            0.5,
            Modifier::BOLD,
        );
        for span in &spans {
            assert!(
                span.style.add_modifier.contains(Modifier::BOLD),
                "every span must carry BOLD modifier"
            );
        }
    }

    #[test]
    fn shimmer_spans_unicode_multibyte() {
        // Unicode text: each char produces one span, not one byte
        let text = "→✓★";
        let spans = shimmer_spans(
            text,
            Color::Rgb(0, 0, 0),
            Color::Rgb(255, 255, 255),
            0.0,
            Modifier::empty(),
        );
        assert_eq!(spans.len(), 3, "3 Unicode chars → 3 spans");
    }
}
