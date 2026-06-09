//! # Step List Pane
//!
//! Left pane of the Install Wizard.
//! Renders an ordered list of [`WizardStep`] items, each with a status glyph
//! and title.  The currently selected step is highlighted.
//!
//! ## Status Glyphs
//!
//! | `StepStatus` | Glyph | Color  |
//! |--------------|-------|--------|
//! | `Ok`         | `✓`   | green  |
//! | `Partial`    | `!`   | yellow |
//! | `Missing`    | `✗`   | red    |
//! | `Pending`    | `…`   | dim    |
//!
//! ## Run-Failed Badge
//!
//! When the most-recent execution for a step ended in `StepExecStatus::Failed`,
//! the step's preflight rollup badge is replaced with a red `✗` run-failed
//! indicator. This makes it immediately clear that *this run* failed,
//! independently of the stale preflight status (which can still read
//! Missing/Partial after a failed run — by design).

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use fdemon_app::install_wizard::{StepStatus, WizardPane, WizardStep, WizardStepKind};

use crate::theme::palette;

/// Glyph for `StepStatus::Ok`.
const GLYPH_OK: &str = "✓";
/// Glyph for `StepStatus::Partial`.
const GLYPH_PARTIAL: &str = "!";
/// Glyph for `StepStatus::Missing`.
const GLYPH_MISSING: &str = "✗";
/// Glyph for `StepStatus::Pending`.
const GLYPH_PENDING: &str = "…";

/// Glyph shown when a step's most-recent execution ended in failure.
///
/// This overrides the preflight rollup badge to make the run failure visually
/// distinct.  Uses the same `✗` codepoint as `GLYPH_MISSING` but is rendered
/// in red regardless of the step's underlying `StepStatus`.
const GLYPH_RUN_FAILED: &str = "✗";

/// Caret shown on the `Platforms` parent row when the submenu is expanded.
///
/// Unicode INVERTED WHITE DOWN-POINTING TRIANGLE (▾).
const CARET_EXPANDED: &str = "▾";

/// Caret shown on the `Platforms` parent row when the submenu is collapsed.
///
/// Unicode BLACK RIGHT-POINTING SMALL TRIANGLE (▸).
const CARET_COLLAPSED: &str = "▸";

/// Extra leading spaces added per indent level.
///
/// Each indent level (currently only level 1 is used for platform leaves)
/// prepends this many spaces before the status glyph, visually nesting the
/// leaf row under the `Platforms` parent.
const INDENT_WIDTH: usize = 2;

/// Height of the pane title header (label + separator).
///
/// Derived from: 1 title row + 1 separator row = 2 rows.
pub(super) const HEADER_HEIGHT: u16 = 2;

/// Left pane — ordered step list.
pub struct StepListPane<'a> {
    steps: &'a [WizardStep],
    selected_index: usize,
    focused: bool,
    /// The step kind whose most-recent execution ended in failure, if any.
    ///
    /// When `Some(kind)`, the badge for that step is rendered as the run-failed
    /// indicator (`GLYPH_RUN_FAILED`, red) rather than the preflight rollup badge.
    failed_step_kind: Option<WizardStepKind>,
    /// Whether the `Platforms` submenu is currently expanded.
    ///
    /// When `true`, the `Platforms` parent row shows `▾` (expanded caret).
    /// When `false`, it shows `▸` (collapsed caret).
    platforms_expanded: bool,
}

impl<'a> StepListPane<'a> {
    /// Create a new step list pane.
    ///
    /// # Arguments
    /// * `steps`             – Ordered list of wizard steps
    /// * `selected_index`    – Currently selected step index
    /// * `focused`           – Whether this pane has keyboard focus
    /// * `failed_step_kind`  – Step kind whose last execution failed (badge override)
    /// * `platforms_expanded` – Whether the Platforms submenu is expanded
    pub fn new(
        steps: &'a [WizardStep],
        selected_index: usize,
        focused: bool,
        failed_step_kind: Option<WizardStepKind>,
        platforms_expanded: bool,
    ) -> Self {
        Self {
            steps,
            selected_index,
            focused,
            failed_step_kind,
            platforms_expanded,
        }
    }

    /// Return the glyph character for the given step status.
    fn status_glyph(status: StepStatus) -> &'static str {
        match status {
            StepStatus::Ok => GLYPH_OK,
            StepStatus::Partial => GLYPH_PARTIAL,
            StepStatus::Missing => GLYPH_MISSING,
            StepStatus::Pending => GLYPH_PENDING,
        }
    }

    /// Return the color for the given step status.
    fn status_color(status: StepStatus) -> ratatui::style::Color {
        match status {
            StepStatus::Ok => palette::STATUS_GREEN,
            StepStatus::Partial => palette::STATUS_YELLOW,
            StepStatus::Missing => palette::STATUS_RED,
            StepStatus::Pending => palette::TEXT_MUTED,
        }
    }

    /// Render the pane header: title + separator underline.
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        let title_style = if self.focused {
            Style::default()
                .fg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::TEXT_SECONDARY)
        };

        let label = Line::from(vec![
            Span::raw("  "),
            Span::styled("Setup Steps", title_style),
        ]);
        Paragraph::new(label).render(Rect::new(area.x, area.y, area.width, 1), buf);

        if area.height >= 2 {
            let separator = "\u{2500}".repeat(area.width as usize); // ─
            buf.set_string(
                area.x,
                area.y + 1,
                &separator,
                Style::default().fg(palette::BORDER_DIM),
            );
        }
    }

    /// Render a single step row at absolute `y` position.
    fn render_step_row(
        &self,
        step: &WizardStep,
        index: usize,
        y: u16,
        area: Rect,
        buf: &mut Buffer,
    ) {
        let is_selected = index == self.selected_index;

        // Run-failed badge takes precedence over the preflight rollup badge.
        // This makes it unambiguous that *this run* failed, independent of the
        // stale preflight status (which can still read Missing/Partial after a
        // failed run — by design).
        let run_failed = self.failed_step_kind == Some(step.kind);

        let (glyph, glyph_color) = if run_failed {
            (GLYPH_RUN_FAILED, palette::STATUS_RED)
        } else {
            (
                Self::status_glyph(step.status),
                Self::status_color(step.status),
            )
        };

        let row_style = if is_selected && self.focused {
            Style::default()
                .fg(palette::CONTRAST_FG)
                .bg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else if is_selected {
            Style::default()
                .fg(palette::TEXT_BRIGHT)
                .bg(palette::SELECTED_ROW_BG)
        } else {
            Style::default().fg(palette::TEXT_PRIMARY)
        };

        let glyph_style = if is_selected && self.focused {
            // Use row style for glyph too when fully selected+focused.
            // BOLD is always applied in this branch (it is already present in
            // the base row_style) so no special run_failed guard is needed.
            Style::default()
                .fg(palette::CONTRAST_FG)
                .bg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else if is_selected {
            let style = Style::default()
                .fg(glyph_color)
                .bg(palette::SELECTED_ROW_BG);
            // BOLD for run-failed badge to distinguish from plain Missing.
            if run_failed {
                style.add_modifier(Modifier::BOLD)
            } else {
                style
            }
        } else {
            let style = Style::default().fg(glyph_color);
            // BOLD for run-failed badge to distinguish from plain Missing (F11).
            // A plain Missing step never has BOLD; this is the only distinguishing
            // attribute when both glyphs are STATUS_RED.
            if run_failed {
                style.add_modifier(Modifier::BOLD)
            } else {
                style
            }
        };

        // Indent: each indent level adds INDENT_WIDTH leading spaces before the glyph.
        // Level 0 = top-level / parent rows (standard "  " prefix).
        // Level 1 = platform leaf rows ("    " prefix — 2 base + 2 indent).
        let indent_spaces = INDENT_WIDTH * step.indent as usize;
        let leading = " ".repeat(2 + indent_spaces);

        // Expand/collapse caret appended to the Platforms parent row title.
        // The caret is NOT included in the selection-highlight width math; it is
        // appended as plain unstyled text so it does not affect the row_style fill.
        let caret = if step.kind == WizardStepKind::Platforms {
            if self.platforms_expanded {
                Some(CARET_EXPANDED)
            } else {
                Some(CARET_COLLAPSED)
            }
        } else {
            None
        };

        let mut spans = vec![
            Span::raw(leading.clone()),
            Span::styled(glyph, glyph_style),
            Span::raw(" "),
            Span::styled(step.title.as_str(), row_style),
        ];
        if let Some(c) = caret {
            // Space before caret keeps it visually separated from the title.
            spans.push(Span::raw(" "));
            spans.push(Span::styled(c, row_style));
        }

        let line = Line::from(spans);

        let row_area = Rect::new(area.x, y, area.width, 1);
        Paragraph::new(line).render(row_area, buf);

        // Fill rest of row with row background to avoid stray characters.
        // text_len counts leading spaces + glyph + space + title.
        // The caret (and its preceding space) are NOT counted here — Paragraph
        // renders them with the row_style already applied, so there is no stray
        // unstyled cell after them; the fill only needs to cover the gap between
        // the end of the title and the right edge of the row.
        if is_selected {
            let text_len =
                (leading.chars().count() + glyph.chars().count() + 1 + step.title.chars().count())
                    as u16;
            // Account for the caret suffix in the fill width to avoid a stray
            // highlighted cell at the right edge of the row.
            let suffix_len = caret.map(|c| 1 + c.chars().count() as u16).unwrap_or(0);
            let used = text_len + suffix_len;
            if used < area.width {
                let padding = " ".repeat((area.width - used) as usize);
                buf.set_string(area.x + used, y, &padding, row_style);
            }
        }
    }
}

impl Widget for StepListPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_header(area, buf);

        let content_y = area.y + HEADER_HEIGHT;
        if content_y >= area.y + area.height {
            return; // No space for content
        }
        let content_area = Rect::new(
            area.x,
            content_y,
            area.width,
            area.height.saturating_sub(HEADER_HEIGHT),
        );

        if self.steps.is_empty() {
            let msg = Line::from(Span::styled(
                "  No steps available.",
                Style::default().fg(palette::TEXT_MUTED),
            ));
            Paragraph::new(msg).render(
                Rect::new(content_area.x, content_area.y, content_area.width, 1),
                buf,
            );
            return;
        }

        let visible_height = content_area.height as usize;
        for (i, step) in self.steps.iter().take(visible_height).enumerate() {
            let y = content_area.y + i as u16;
            self.render_step_row(step, i, y, content_area, buf);
        }
    }
}

/// Construct a [`StepListPane`] from the install wizard state fields.
///
/// `failed_step_kind` should be set to the execution's `kind` when the execution
/// status is `StepExecStatus::Failed`, so the step-list badge shows the
/// run-failed indicator for that step.  Pass `None` otherwise.
///
/// `platforms_expanded` mirrors `InstallWizardState::platforms_expanded` and
/// controls whether the `Platforms` parent row shows the `▾` or `▸` caret.
///
/// This is a convenience constructor used by [`super::InstallWizardPanel`].
pub fn step_list_pane<'a>(
    steps: &'a [WizardStep],
    selected_index: usize,
    focused_pane: WizardPane,
    failed_step_kind: Option<WizardStepKind>,
    platforms_expanded: bool,
) -> StepListPane<'a> {
    StepListPane::new(
        steps,
        selected_index,
        focused_pane == WizardPane::StepList,
        failed_step_kind,
        platforms_expanded,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::install_wizard::{StepStatus, WizardStepKind};
    use ratatui::{buffer::Buffer, layout::Rect};

    /// Build a minimal flat step list (all top-level, no platform leaves).
    ///
    /// Keeps coordinates stable across collapsed / expanded state — use this
    /// helper when the test only needs to exercise top-level steps.
    fn make_steps() -> Vec<WizardStep> {
        vec![
            WizardStep {
                kind: WizardStepKind::Prerequisites,
                title: "Prerequisites".to_string(),
                status: StepStatus::Ok,
                components: vec![],
                guided_commands: vec![],
                indent: 0,
            },
            WizardStep {
                kind: WizardStepKind::PlatformAndroid,
                title: "Android".to_string(),
                status: StepStatus::Partial,
                components: vec![],
                guided_commands: vec![],
                indent: 1,
            },
            WizardStep {
                kind: WizardStepKind::FlutterSdk,
                title: "Flutter SDK".to_string(),
                status: StepStatus::Missing,
                components: vec![],
                guided_commands: vec![],
                indent: 0,
            },
            WizardStep {
                kind: WizardStepKind::Doctor,
                title: "Flutter Doctor".to_string(),
                status: StepStatus::Pending,
                components: vec![],
                guided_commands: vec![],
                indent: 0,
            },
        ]
    }

    /// Build a minimal step list with the Platforms parent row (collapsed state).
    ///
    /// Index 0 = Prerequisites (indent=0), index 1 = Platforms parent (indent=0).
    fn make_steps_with_platforms_parent() -> Vec<WizardStep> {
        vec![
            WizardStep {
                kind: WizardStepKind::Prerequisites,
                title: "Prerequisites".to_string(),
                status: StepStatus::Ok,
                components: vec![],
                guided_commands: vec![],
                indent: 0,
            },
            WizardStep {
                kind: WizardStepKind::Platforms,
                title: "Platforms".to_string(),
                status: StepStatus::Partial,
                components: vec![],
                guided_commands: vec![],
                indent: 0,
            },
            WizardStep {
                kind: WizardStepKind::FlutterSdk,
                title: "Flutter SDK".to_string(),
                status: StepStatus::Missing,
                components: vec![],
                guided_commands: vec![],
                indent: 0,
            },
        ]
    }

    /// Build a step list with Platforms parent + one expanded leaf.
    fn make_steps_with_expanded_platforms() -> Vec<WizardStep> {
        vec![
            WizardStep {
                kind: WizardStepKind::Prerequisites,
                title: "Prerequisites".to_string(),
                status: StepStatus::Ok,
                components: vec![],
                guided_commands: vec![],
                indent: 0,
            },
            WizardStep {
                kind: WizardStepKind::Platforms,
                title: "Platforms".to_string(),
                status: StepStatus::Partial,
                components: vec![],
                guided_commands: vec![],
                indent: 0,
            },
            WizardStep {
                kind: WizardStepKind::PlatformAndroid,
                title: "Android".to_string(),
                status: StepStatus::Partial,
                components: vec![],
                guided_commands: vec![],
                indent: 1,
            },
            WizardStep {
                kind: WizardStepKind::FlutterSdk,
                title: "Flutter SDK".to_string(),
                status: StepStatus::Missing,
                components: vec![],
                guided_commands: vec![],
                indent: 0,
            },
        ]
    }

    fn make_area() -> Rect {
        Rect::new(0, 0, 40, 20)
    }

    #[test]
    fn test_renders_step_list_with_status_glyphs() {
        let steps = make_steps();
        let pane = StepListPane::new(&steps, 0, true, None, false);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(content.contains('✓'), "Ok step should show checkmark glyph");
        assert!(content.contains('!'), "Partial step should show ! glyph");
        assert!(content.contains('✗'), "Missing step should show ✗ glyph");
        assert!(content.contains('…'), "Pending step should show … glyph");
    }

    #[test]
    fn test_renders_step_titles() {
        let steps = make_steps();
        let pane = StepListPane::new(&steps, 0, true, None, false);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("Prerequisites"),
            "should show first step title"
        );
        assert!(
            content.contains("Android"),
            "should show android platform leaf title"
        );
        assert!(
            content.contains("Flutter SDK"),
            "should show flutter sdk title"
        );
        assert!(
            content.contains("Flutter Doctor"),
            "should show doctor title"
        );
    }

    #[test]
    fn test_selected_step_highlighted() {
        // Use a top-level step (Prerequisites, index 0, indent=0) so the glyph
        // x-coordinate is stable regardless of any platform leaf expansion.
        let steps = make_steps();
        let pane = StepListPane::new(&steps, 0, true, None, false);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        // Row at y = HEADER_HEIGHT(2) + index(0) = 2 should have accent background.
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Prerequisites"),
            "selected step title should appear"
        );
        // Glyph cell: x=2 (two leading spaces, indent=0), y=2.
        let cell = &buf[(2, 2)];
        assert_eq!(
            cell.bg,
            palette::ACCENT,
            "selected+focused row should have accent background"
        );
    }

    #[test]
    fn test_unfocused_selected_uses_subtle_highlight() {
        let steps = make_steps();
        let pane = StepListPane::new(&steps, 0, false, None, false); // not focused
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        // Step 0 is selected but pane is unfocused — should use SELECTED_ROW_BG
        // Row y = header(2) + 0 = 2
        let cell = &buf[(2, 2)];
        assert_eq!(
            cell.bg,
            palette::SELECTED_ROW_BG,
            "selected but unfocused row should use subtle highlight"
        );
    }

    #[test]
    fn test_renders_without_panic_empty_steps() {
        let steps: Vec<WizardStep> = vec![];
        let pane = StepListPane::new(&steps, 0, true, None, false);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_renders_without_panic_tiny_area() {
        let steps = make_steps();
        let pane = StepListPane::new(&steps, 0, true, None, false);
        let area = Rect::new(0, 0, 5, 2);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_header_shows_in_focused_state() {
        let steps = make_steps();
        let pane = StepListPane::new(&steps, 0, true, None, false);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Setup Steps"),
            "header label should appear"
        );
    }

    // --- Phase 5 task 06: run-failed badge tests ---

    #[test]
    fn step_list_shows_failed_indicator_after_failed_execution() {
        // FlutterSdk step (index 2, status=Missing) — execution failed → badge
        // must be red ✗.
        // Select index 0 (not FlutterSdk) so the FlutterSdk row is NOT
        // selected+focused; that lets us observe the run-failed badge colour
        // rather than the accent-row override.
        //
        // make_steps() layout: [0]=Prerequisites(indent=0), [1]=PlatformAndroid(indent=1),
        // [2]=FlutterSdk(indent=0), [3]=Doctor(indent=0).
        // FlutterSdk glyph x = 2 (indent=0 → 2 leading spaces), y = HEADER_HEIGHT(2)+2 = 4.
        let steps = make_steps();
        let pane = StepListPane::new(&steps, 0, true, Some(WizardStepKind::FlutterSdk), false);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);

        // FlutterSdk row is at y = HEADER_HEIGHT(2) + index(2) = 4.
        // The glyph cell is at x=2 (two leading spaces, indent=0), y=4.
        // The step is NOT selected (selected_index=0), so the glyph colour is
        // the badge colour — STATUS_RED for run-failed.
        let glyph_cell = &buf[(2, 4)];
        assert_eq!(
            glyph_cell.fg,
            palette::STATUS_RED,
            "run-failed glyph should be red; got {:?}",
            glyph_cell.fg
        );
        // Confirm the glyph symbol is ✗
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains('✗'),
            "run-failed indicator glyph (✗) must appear: '{content}'"
        );
    }

    #[test]
    fn step_list_failed_badge_does_not_affect_other_steps() {
        // Execution failed for FlutterSdk (index 2); other steps must retain
        // their preflight rollup badges.
        // Select index 2 (FlutterSdk) as current, so the other step rows are
        // NOT selected+focused and we can read their unmodified badge colours.
        //
        // make_steps() layout: [0]=Prerequisites(indent=0), [1]=PlatformAndroid(indent=1),
        // [2]=FlutterSdk(indent=0). FlutterSdk glyph x=2, y=4.
        // PlatformAndroid glyph x = 2 + INDENT_WIDTH*1 = 4, y = HEADER_HEIGHT(2) + 1 = 3.
        let steps = make_steps();
        let pane = StepListPane::new(&steps, 2, true, Some(WizardStepKind::FlutterSdk), false);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);

        // Prerequisites row (index 0) is at y = HEADER_HEIGHT(2) + 0 = 2; glyph x=2 (indent=0).
        // Status=Ok → glyph should be STATUS_GREEN (unselected row, no run-failed).
        let prereq_glyph_cell = &buf[(2, 2)];
        assert_eq!(
            prereq_glyph_cell.fg,
            palette::STATUS_GREEN,
            "Prerequisites (Ok) badge should stay green; run-failed only applies to FlutterSdk"
        );

        // PlatformAndroid row (index 1) at y=3; indent=1 → glyph at x = 2 + 2 = 4.
        // Status=Partial → STATUS_YELLOW.
        let android_glyph_cell = &buf[(4, 3)];
        assert_eq!(
            android_glyph_cell.fg,
            palette::STATUS_YELLOW,
            "PlatformAndroid (Partial) badge should stay yellow; run-failed only applies to FlutterSdk"
        );
    }

    #[test]
    fn step_list_no_failed_badge_when_execution_is_none() {
        // With failed_step_kind=None the Missing FlutterSdk step shows STATUS_RED
        // (its normal preflight badge) rather than a run-failed override.
        // Select index 0 to keep FlutterSdk unselected so its glyph colour is
        // directly observable.
        //
        // FlutterSdk is at index 2, indent=0, so glyph x=2, y = HEADER_HEIGHT(2)+2 = 4.
        let steps = make_steps();
        let pane = StepListPane::new(&steps, 0, true, None, false);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        // FlutterSdk (index 2, Missing) unselected → STATUS_RED fg.
        let cell = &buf[(2, 4)];
        assert_eq!(
            cell.fg,
            palette::STATUS_RED,
            "Missing step should show STATUS_RED badge when no run-failed override"
        );
    }

    // --- Task 03 (F11): run-failed badge BOLD vs plain Missing distinctness ---

    /// F11: The run-failed badge must have `Modifier::BOLD`; a plain `Missing`
    /// badge (same codepoint, same colour) must NOT have `Modifier::BOLD`.
    /// This is the only distinguishing attribute when both are `STATUS_RED`.
    #[test]
    fn run_failed_badge_is_bold_plain_missing_is_not() {
        // steps[2] = FlutterSdk, status=Missing, indent=0
        // With failed_step_kind=Some(FlutterSdk) its glyph gets BOLD.
        // With failed_step_kind=None its glyph does NOT get BOLD.
        // Select index 0 (Prerequisites) so FlutterSdk row is unselected in both cases
        // — the unselected branch is where the BOLD difference is visible.
        // FlutterSdk glyph x=2 (indent=0), y = HEADER_HEIGHT(2) + index(2) = 4.
        let steps = make_steps();
        let area = make_area();

        // --- Run-failed: BOLD expected ---
        let mut buf_failed = Buffer::empty(area);
        let pane_failed =
            StepListPane::new(&steps, 0, true, Some(WizardStepKind::FlutterSdk), false);
        pane_failed.render(area, &mut buf_failed);
        // FlutterSdk glyph at x=2, y = HEADER_HEIGHT(2) + index(2) = 4
        let run_failed_cell = &buf_failed[(2, 4)];
        assert!(
            run_failed_cell
                .style()
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD),
            "run-failed glyph must have Modifier::BOLD; modifiers: {:?}",
            run_failed_cell.style().add_modifier
        );

        // --- Plain Missing: BOLD NOT expected ---
        let mut buf_plain = Buffer::empty(area);
        let pane_plain = StepListPane::new(&steps, 0, true, None, false);
        pane_plain.render(area, &mut buf_plain);
        let plain_missing_cell = &buf_plain[(2, 4)];
        assert!(
            !plain_missing_cell
                .style()
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD),
            "plain Missing glyph must NOT have Modifier::BOLD; modifiers: {:?}",
            plain_missing_cell.style().add_modifier
        );
    }

    // --- Task 03 (Phase 2): expand/collapse caret and leaf indent tests ---

    /// The Platforms parent row must show the collapsed caret (▸) when
    /// `platforms_expanded = false`.
    #[test]
    fn platforms_parent_shows_collapsed_caret() {
        let steps = make_steps_with_platforms_parent();
        let pane = StepListPane::new(&steps, 0, true, None, false);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains(CARET_COLLAPSED),
            "collapsed Platforms parent must show '▸' caret; content: {content:?}"
        );
        assert!(
            !content.contains(CARET_EXPANDED),
            "collapsed Platforms parent must NOT show '▾' caret; content: {content:?}"
        );
    }

    /// The Platforms parent row must show the expanded caret (▾) when
    /// `platforms_expanded = true`.
    #[test]
    fn platforms_parent_shows_expanded_caret() {
        let steps = make_steps_with_platforms_parent();
        let pane = StepListPane::new(&steps, 0, true, None, true);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains(CARET_EXPANDED),
            "expanded Platforms parent must show '▾' caret; content: {content:?}"
        );
        assert!(
            !content.contains(CARET_COLLAPSED),
            "expanded Platforms parent must NOT show '▸' caret; content: {content:?}"
        );
    }

    /// A leaf row (indent=1) must have its glyph at a greater x-offset than a
    /// top-level row (indent=0) in the same rendered buffer.
    #[test]
    fn leaf_row_glyph_is_indented_relative_to_top_level() {
        // steps: [0]=Prerequisites(indent=0), [1]=Platforms(indent=0),
        //        [2]=PlatformAndroid(indent=1), [3]=FlutterSdk(indent=0).
        let steps = make_steps_with_expanded_platforms();
        let pane = StepListPane::new(&steps, 0, true, None, true);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf);

        // Top-level glyph x: Prerequisites at index 0, indent=0 → x = 2.
        // Leaf glyph x: PlatformAndroid at index 2, indent=1 → x = 2 + INDENT_WIDTH = 4.
        let top_level_glyph_x: u16 = 2;
        let leaf_glyph_x: u16 = 2 + INDENT_WIDTH as u16;
        let top_y: u16 = HEADER_HEIGHT; // y=2
        let leaf_y: u16 = HEADER_HEIGHT + 2; // y=4 (Platforms at 1, Android at 2)

        // Top-level row glyph: ✓ at (2, 2)
        let top_cell = &buf[(top_level_glyph_x, top_y)];
        assert_eq!(
            top_cell.symbol(),
            GLYPH_OK,
            "top-level glyph at x={top_level_glyph_x}, y={top_y} should be '{GLYPH_OK}'; got: {:?}",
            top_cell.symbol()
        );

        // Leaf row glyph: ! at (4, 4) — PlatformAndroid is Partial
        let leaf_cell = &buf[(leaf_glyph_x, leaf_y)];
        assert_eq!(
            leaf_cell.symbol(),
            GLYPH_PARTIAL,
            "leaf glyph at x={leaf_glyph_x}, y={leaf_y} should be '{GLYPH_PARTIAL}'; got: {:?}",
            leaf_cell.symbol()
        );

        assert!(
            leaf_glyph_x > top_level_glyph_x,
            "leaf glyph x ({leaf_glyph_x}) must be greater than top-level glyph x ({top_level_glyph_x})"
        );
    }

    /// The step-list widget must render more rows when a leaf is present
    /// (expanded) vs. absent (collapsed), because the visible-step count grows.
    /// This test verifies that the leaf title appears only in the expanded fixture.
    #[test]
    fn step_list_height_grows_when_expanded() {
        let collapsed_steps = make_steps_with_platforms_parent(); // 3 steps
        let expanded_steps = make_steps_with_expanded_platforms(); // 4 steps

        let area = make_area();

        // Collapsed: PlatformAndroid leaf title must NOT appear
        let mut buf_collapsed = Buffer::empty(area);
        StepListPane::new(&collapsed_steps, 0, true, None, false).render(area, &mut buf_collapsed);
        let collapsed_content: String =
            buf_collapsed.content().iter().map(|c| c.symbol()).collect();
        assert!(
            !collapsed_content.contains("Android"),
            "collapsed step list must not render the leaf title; content: {collapsed_content:?}"
        );

        // Expanded: PlatformAndroid leaf title MUST appear
        let mut buf_expanded = Buffer::empty(area);
        StepListPane::new(&expanded_steps, 0, true, None, true).render(area, &mut buf_expanded);
        let expanded_content: String = buf_expanded.content().iter().map(|c| c.symbol()).collect();
        assert!(
            expanded_content.contains("Android"),
            "expanded step list must render the leaf title; content: {expanded_content:?}"
        );
    }
}
