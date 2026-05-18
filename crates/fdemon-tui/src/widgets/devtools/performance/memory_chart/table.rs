//! Allocation table renderer for the memory chart.
//!
//! Renders the class allocation table below the chart, showing
//! the top classes sorted by either total size or total instance count.
//!
//! The [`AllocationTable`] struct supports scrollable windowing, selected-row
//! highlighting, and mouse click region registration. The legacy free-function
//! [`render_allocation_table`] is preserved as a thin wrapper for compatibility
//! with existing tests.

use std::cell::Cell;
use std::cmp::Reverse;

use fdemon_app::session::AllocationSortColumn;
use fdemon_app::session::PerfSection;
use fdemon_app::MouseRect;
use fdemon_app::{Message, MouseAction};
use fdemon_core::performance::ClassHeapStats;

use super::*;

// ── Layout constants ──────────────────────────────────────────────────────────

/// Number of rows consumed by the table header: one header line + one separator line.
pub(super) const TABLE_HEADER_ROWS: usize = 2;

/// Column width for the class name field.
const CLASS_NAME_WIDTH: usize = 30;

/// Column width for the instances field.
const INSTANCES_WIDTH: usize = 12;

/// Column width for the size field.
const SIZE_WIDTH: usize = 14;

// ── AllocationTable ───────────────────────────────────────────────────────────

/// Scrollable, selectable allocation table widget.
///
/// Renders a windowed slice of `profile.members`, sorted by `sort_column`,
/// with optional row highlight for the selected row and click region
/// registration.
///
/// # Layout
///
/// ```text
/// Class                          Instances   Shallow Size
/// ──────────────────────────────────────────────────────
/// dart:core/String                  12,345         2.4 MB  ← row 0 (global index = scroll_offset)
/// dart:core/_List                    2,000       400.0 KB
/// …
/// ```
pub(super) struct AllocationTable<'a> {
    pub(super) profile: &'a AllocationProfile,
    pub(super) sort_column: AllocationSortColumn,
    /// First row of the full sorted list to render.
    pub(super) scroll_offset: usize,
    /// Global index (into the full sorted list) of the currently selected row.
    pub(super) selected_row: Option<usize>,
    /// Whether the allocation table section currently has keyboard focus.
    // Used for future focused-border styling; annotated to suppress dead_code warning.
    #[allow(dead_code)]
    pub(super) focused: bool,
    /// Render-hint Cell written every frame with the number of data rows that
    /// fit in the current area.
    // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md "Region Registry Pattern".
    pub(super) visible_height_cell: &'a Cell<usize>,
}

impl AllocationTable<'_> {
    /// Render the allocation table into `area`, optionally registering click regions.
    pub(super) fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        mut mouse: Option<&mut crate::widgets::MouseCtx<'_>>,
    ) {
        if area.height == 0 || area.width < 10 {
            // Not enough space — write 0 to the render-hint and return.
            self.visible_height_cell.set(0);
            return;
        }

        // ── Render header ─────────────────────────────────────────────────────

        let (instances_label, size_label) = match self.sort_column {
            AllocationSortColumn::BySize => (
                format!("{:>width$}", "Instances", width = INSTANCES_WIDTH),
                format!(
                    "{:>width$}",
                    "Shallow Size \u{25bc}",
                    width = SIZE_WIDTH + 2
                ),
            ),
            AllocationSortColumn::ByInstances => (
                format!(
                    "{:>width$}",
                    "Instances \u{25bc}",
                    width = INSTANCES_WIDTH + 2
                ),
                format!("{:>width$}", "Shallow Size", width = SIZE_WIDTH),
            ),
        };

        let header_line = Line::from(vec![
            Span::styled(
                format!("{:<width$}", "Class", width = CLASS_NAME_WIDTH),
                Style::default().fg(palette::TEXT_SECONDARY),
            ),
            Span::styled(
                instances_label,
                Style::default().fg(palette::TEXT_SECONDARY),
            ),
            Span::styled(size_label, Style::default().fg(palette::TEXT_SECONDARY)),
        ]);
        buf.set_line(area.x, area.y, &header_line, area.width);

        if area.height < 2 {
            self.visible_height_cell.set(0);
            return;
        }

        // ── Render separator ──────────────────────────────────────────────────

        let sep: String = "\u{2500}".repeat(area.width as usize);
        let sep_line = Line::from(Span::styled(sep, Style::default().fg(palette::BORDER_DIM)));
        buf.set_line(area.x, area.y + 1, &sep_line, area.width);

        // ── Compute visible height ─────────────────────────────────────────────

        let visible_height = (area.height as usize).saturating_sub(TABLE_HEADER_ROWS);
        // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md "Region Registry Pattern".
        self.visible_height_cell.set(visible_height);

        if visible_height == 0 {
            return;
        }

        let data_start_y = area.y + TABLE_HEADER_ROWS as u16;

        // ── Sort + window ─────────────────────────────────────────────────────

        let mut sorted: Vec<&ClassHeapStats> = self.profile.members.iter().collect();
        match self.sort_column {
            AllocationSortColumn::BySize => {
                sorted.sort_by_key(|s| Reverse(s.total_size()));
            }
            AllocationSortColumn::ByInstances => {
                sorted.sort_by_key(|s| Reverse(s.total_instances()));
            }
        }

        if sorted.is_empty() {
            let msg = Line::from(Span::styled(
                "No class allocations reported",
                Style::default().fg(palette::TEXT_SECONDARY),
            ));
            buf.set_line(area.x, data_start_y, &msg, area.width);
            return;
        }

        let scroll = self.scroll_offset.min(sorted.len().saturating_sub(1));
        let end = (scroll + visible_height).min(sorted.len());
        let visible_slice = &sorted[scroll..end];

        // ── Render data rows ──────────────────────────────────────────────────

        for (row_idx, class) in visible_slice.iter().enumerate() {
            let global_idx = scroll + row_idx;
            let row_y = data_start_y + row_idx as u16;
            if row_y >= area.bottom() {
                break;
            }

            // Truncate class name to CLASS_NAME_WIDTH chars (char-aware to avoid
            // panic on multi-byte UTF-8 codepoints such as CJK or emoji).
            let name = if class.class_name.chars().count() > CLASS_NAME_WIDTH {
                format!(
                    "{}...",
                    class
                        .class_name
                        .chars()
                        .take(CLASS_NAME_WIDTH - 3)
                        .collect::<String>()
                )
            } else {
                class.class_name.clone()
            };

            let (name_style, num_style) = if Some(global_idx) == self.selected_row {
                // Highlight the selected row with the accent colour as background.
                let bg = palette::ACCENT;
                let fg = palette::DEEPEST_BG;
                (
                    Style::default().bg(bg).fg(fg),
                    Style::default().bg(bg).fg(fg),
                )
            } else {
                (
                    Style::default().fg(palette::TEXT_PRIMARY),
                    Style::default().fg(palette::TEXT_SECONDARY),
                )
            };

            let row = Line::from(vec![
                Span::styled(
                    format!("{:<width$}", name, width = CLASS_NAME_WIDTH),
                    name_style,
                ),
                Span::styled(
                    format!(
                        "{:>width$}",
                        format_number(class.total_instances()),
                        width = INSTANCES_WIDTH
                    ),
                    num_style,
                ),
                Span::styled(
                    format!(
                        "{:>width$}",
                        MemoryUsage::format_bytes(class.total_size()),
                        width = SIZE_WIDTH
                    ),
                    num_style,
                ),
            ]);
            buf.set_line(area.x, row_y, &row, area.width);

            // Register per-row click region.
            if let Some(ctx) = mouse.as_deref_mut() {
                let row_rect = MouseRect::new(area.x, row_y, area.width, 1);
                ctx.click(
                    row_rect,
                    MouseAction::emit(Message::PerfSelectAllocRow {
                        index: Some(global_idx),
                    }),
                );
            }
        }

        // ── Empty-area focus region ───────────────────────────────────────────
        // Any click below the last visible row focuses the MemoryList section
        // (but does not select a specific row).

        let used_rows = visible_slice.len() as u16;
        let remaining_y = data_start_y + used_rows;
        let remaining_height = area
            .height
            .saturating_sub(TABLE_HEADER_ROWS as u16 + used_rows);

        if remaining_height > 0 {
            if let Some(ctx) = mouse {
                let remaining = MouseRect::new(area.x, remaining_y, area.width, remaining_height);
                // T02 transitional: row click emits a no-op message because PerfSection no longer
                // has MemoryList. T03 will replace this with Message::MemFocusSection(MemorySection::AllocationList).
                ctx.click(
                    remaining,
                    MouseAction::emit(Message::PerfFocusSection(PerfSection::FrameChart)),
                );
            }
        }
    }
}

// ── Legacy free-function wrapper ──────────────────────────────────────────────

/// Render the class allocation table — legacy free-function wrapper.
///
/// Delegates to [`AllocationTable`] with `scroll_offset = 0`, `selected_row = None`,
/// and `focused = false`. Kept for compatibility with existing tests.
pub(super) fn render_allocation_table(
    allocation_profile: Option<&AllocationProfile>,
    sort_column: AllocationSortColumn,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.height == 0 || area.width < 10 {
        return;
    }

    match allocation_profile {
        None => {
            // Build the header
            render_no_profile_header(sort_column, area, buf);

            if area.height >= 3 {
                let data_start_y = area.y + TABLE_HEADER_ROWS as u16;
                let msg = Line::from(Span::styled(
                    "Waiting for allocation data...",
                    Style::default().fg(palette::TEXT_SECONDARY),
                ));
                buf.set_line(area.x, data_start_y, &msg, area.width);
            }
        }
        Some(profile) => {
            let dummy_cell = Cell::new(0usize);
            let table = AllocationTable {
                profile,
                sort_column,
                scroll_offset: 0,
                selected_row: None,
                focused: false,
                visible_height_cell: &dummy_cell,
            };
            table.render(area, buf, None);
        }
    }
}

/// Render the header + separator for a `None` profile (waiting state).
fn render_no_profile_header(sort_column: AllocationSortColumn, area: Rect, buf: &mut Buffer) {
    let (instances_label, size_label) = match sort_column {
        AllocationSortColumn::BySize => (
            format!("{:>width$}", "Instances", width = INSTANCES_WIDTH),
            format!(
                "{:>width$}",
                "Shallow Size \u{25bc}",
                width = SIZE_WIDTH + 2
            ),
        ),
        AllocationSortColumn::ByInstances => (
            format!(
                "{:>width$}",
                "Instances \u{25bc}",
                width = INSTANCES_WIDTH + 2
            ),
            format!("{:>width$}", "Shallow Size", width = SIZE_WIDTH),
        ),
    };

    let header_line = Line::from(vec![
        Span::styled(
            format!("{:<width$}", "Class", width = CLASS_NAME_WIDTH),
            Style::default().fg(palette::TEXT_SECONDARY),
        ),
        Span::styled(
            instances_label,
            Style::default().fg(palette::TEXT_SECONDARY),
        ),
        Span::styled(size_label, Style::default().fg(palette::TEXT_SECONDARY)),
    ]);
    buf.set_line(area.x, area.y, &header_line, area.width);

    if area.height >= 2 {
        let sep: String = "\u{2500}".repeat(area.width as usize);
        let sep_line = Line::from(Span::styled(sep, Style::default().fg(palette::BORDER_DIM)));
        buf.set_line(area.x, area.y + 1, &sep_line, area.width);
    }
}
