//! Allocation table renderer for the memory chart.
//!
//! Renders the class allocation table below the chart, showing
//! the top classes sorted by either total size or total instance count.

use fdemon_app::session::AllocationSortColumn;

use super::*;

// ── Allocation table ──────────────────────────────────────────────────────────

/// Render the class allocation table below the chart.
///
/// The `sort_column` parameter controls which column is used for sorting and
/// receives a `▼` indicator in the header.
pub(super) fn render_allocation_table(
    allocation_profile: Option<&AllocationProfile>,
    sort_column: AllocationSortColumn,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.height == 0 || area.width < 10 {
        return;
    }

    // Build header spans with sort indicator on the active column.
    let (instances_label, size_label) = match sort_column {
        AllocationSortColumn::BySize => (
            format!("{:>12}", "Instances"),
            format!("{:>14}", "Shallow Size \u{25bc}"),
        ),
        AllocationSortColumn::ByInstances => (
            format!("{:>12}", "Instances \u{25bc}"),
            format!("{:>14}", "Shallow Size"),
        ),
    };

    let header_line = Line::from(vec![
        Span::styled(
            format!("{:<30}", "Class"),
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
        return;
    }

    // Separator
    let sep: String = "\u{2500}".repeat(area.width as usize);
    let sep_line = Line::from(Span::styled(sep, Style::default().fg(palette::BORDER_DIM)));
    buf.set_line(area.x, area.y + 1, &sep_line, area.width);

    if area.height < 3 {
        return;
    }

    let data_start_y = area.y + TABLE_HEADER_HEIGHT;
    let available_rows = area.height.saturating_sub(TABLE_HEADER_HEIGHT) as usize;

    match allocation_profile {
        None => {
            let msg = Line::from(Span::styled(
                "Waiting for allocation data...",
                Style::default().fg(palette::TEXT_SECONDARY),
            ));
            buf.set_line(area.x, data_start_y, &msg, area.width);
        }
        Some(profile) => {
            // Sort according to the active column.
            let classes: Vec<_> = match sort_column {
                AllocationSortColumn::BySize => profile.top_by_size(MAX_TABLE_ROWS),
                AllocationSortColumn::ByInstances => {
                    let mut sorted: Vec<_> = profile.members.iter().collect();
                    sorted.sort_by_key(|b| std::cmp::Reverse(b.total_instances()));
                    sorted.truncate(MAX_TABLE_ROWS);
                    sorted
                }
            };

            if classes.is_empty() {
                let msg = Line::from(Span::styled(
                    "No class allocations reported",
                    Style::default().fg(palette::TEXT_SECONDARY),
                ));
                buf.set_line(area.x, data_start_y, &msg, area.width);
                return;
            }
            let display_count = classes.len().min(available_rows);

            for (i, class) in classes.iter().take(display_count).enumerate() {
                let row_y = data_start_y + i as u16;
                if row_y >= area.bottom() {
                    break;
                }

                // Truncate class name to 30 chars (char-aware to avoid panic on multi-byte UTF-8)
                let name = if class.class_name.chars().count() > 30 {
                    format!(
                        "{}...",
                        class.class_name.chars().take(27).collect::<String>()
                    )
                } else {
                    class.class_name.clone()
                };

                let row = Line::from(vec![
                    Span::styled(
                        format!("{:<30}", name),
                        Style::default().fg(palette::TEXT_PRIMARY),
                    ),
                    Span::styled(
                        format!("{:>12}", format_number(class.total_instances())),
                        Style::default().fg(palette::TEXT_SECONDARY),
                    ),
                    Span::styled(
                        format!("{:>14}", MemoryUsage::format_bytes(class.total_size())),
                        Style::default().fg(palette::TEXT_SECONDARY),
                    ),
                ]);
                buf.set_line(area.x, row_y, &row, area.width);
            }
        }
    }
}
