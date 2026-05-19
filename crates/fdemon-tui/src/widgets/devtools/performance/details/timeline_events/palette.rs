//! Color palette for the Gantt-style timeline events widget.
//!
//! Color selection is based on thread classification and nesting depth,
//! mirroring DevTools' two-color-per-thread alternating-depth scheme.
//!
//! Each thread type has a two-entry palette that alternates with nesting depth,
//! so parent and child bars within the same thread row use visually distinct
//! colors.

use fdemon_core::timeline::TimelineThread;
use ratatui::style::Color;

// ── Thread bar colors (two-entry palette per thread, alternating by depth) ────

/// UI thread root-depth (even) bar color — light blue.
const COLOR_UI_EVEN: Color = Color::LightBlue;
/// UI thread odd-depth bar color — blue.
const COLOR_UI_ODD: Color = Color::Blue;

/// Raster thread root-depth (even) bar color — blue.
const COLOR_RASTER_EVEN: Color = Color::Blue;
/// Raster thread odd-depth bar color — dark gray.
const COLOR_RASTER_ODD: Color = Color::DarkGray;

/// Other/worker thread root-depth (even) bar color — magenta.
const COLOR_OTHER_EVEN: Color = Color::Magenta;
/// Other/worker thread odd-depth bar color — light magenta.
const COLOR_OTHER_ODD: Color = Color::LightMagenta;

// ── Thread label colors ───────────────────────────────────────────────────────

/// Label color for UI thread rows.
const COLOR_LABEL_UI: Color = Color::LightBlue;
/// Label color for Raster thread rows.
const COLOR_LABEL_RASTER: Color = Color::Blue;
/// Label color for Other thread rows.
const COLOR_LABEL_OTHER: Color = Color::Magenta;

// ── Public API ────────────────────────────────────────────────────────────────

/// Return the bar fill color for the given thread type and nesting depth.
///
/// Color alternates with depth (even depth vs. odd depth) to visually
/// distinguish parent/child event bars within the same thread row, mirroring
/// the DevTools flame chart convention.
pub(super) fn bar_color(thread: TimelineThread, depth: u8) -> Color {
    let is_even = depth.is_multiple_of(2);
    match thread {
        TimelineThread::Ui => {
            if is_even {
                COLOR_UI_EVEN
            } else {
                COLOR_UI_ODD
            }
        }
        TimelineThread::Raster => {
            if is_even {
                COLOR_RASTER_EVEN
            } else {
                COLOR_RASTER_ODD
            }
        }
        TimelineThread::Other => {
            if is_even {
                COLOR_OTHER_EVEN
            } else {
                COLOR_OTHER_ODD
            }
        }
    }
}

/// Return the thread-label text color for the given thread type.
pub(super) fn label_color(thread: TimelineThread) -> Color {
    match thread {
        TimelineThread::Ui => COLOR_LABEL_UI,
        TimelineThread::Raster => COLOR_LABEL_RASTER,
        TimelineThread::Other => COLOR_LABEL_OTHER,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_color_ui_depth_0_is_light_blue() {
        assert_eq!(bar_color(TimelineThread::Ui, 0), Color::LightBlue);
    }

    #[test]
    fn bar_color_ui_depth_1_is_blue() {
        assert_eq!(bar_color(TimelineThread::Ui, 1), Color::Blue);
    }

    #[test]
    fn bar_color_raster_depth_0_is_blue() {
        assert_eq!(bar_color(TimelineThread::Raster, 0), Color::Blue);
    }

    #[test]
    fn bar_color_raster_depth_1_is_dark_gray() {
        assert_eq!(bar_color(TimelineThread::Raster, 1), Color::DarkGray);
    }

    #[test]
    fn bar_color_other_depth_0_is_magenta() {
        assert_eq!(bar_color(TimelineThread::Other, 0), Color::Magenta);
    }

    #[test]
    fn bar_color_other_depth_1_is_light_magenta() {
        assert_eq!(bar_color(TimelineThread::Other, 1), Color::LightMagenta);
    }

    #[test]
    fn bar_color_alternates_with_depth() {
        // depth=2 wraps back to same color as depth=0
        assert_eq!(
            bar_color(TimelineThread::Ui, 2),
            bar_color(TimelineThread::Ui, 0)
        );
        assert_eq!(
            bar_color(TimelineThread::Ui, 3),
            bar_color(TimelineThread::Ui, 1)
        );
    }

    #[test]
    fn label_color_ui_is_light_blue() {
        assert_eq!(label_color(TimelineThread::Ui), Color::LightBlue);
    }

    #[test]
    fn label_color_raster_is_blue() {
        assert_eq!(label_color(TimelineThread::Raster), Color::Blue);
    }

    #[test]
    fn label_color_other_is_magenta() {
        assert_eq!(label_color(TimelineThread::Other), Color::Magenta);
    }
}
