//! Confirmation dialog widget for quit/close confirmations

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

// Import state from app layer (re-exported at widgets/mod.rs level)
use fdemon_app::confirm_dialog::ConfirmDialogState;
use fdemon_app::{MouseAction, MouseRect};

use crate::theme::palette;
use crate::widgets::modal_overlay::centered_rect;
use crate::widgets::MouseCtx;

/// Confirmation dialog widget
pub struct ConfirmDialog<'a> {
    state: &'a ConfirmDialogState,
}

impl<'a> ConfirmDialog<'a> {
    /// Create a new confirmation dialog widget
    pub fn new(state: &'a ConfirmDialogState) -> Self {
        Self { state }
    }
}

/// Render `ConfirmDialog` and record clickable button regions.
///
/// This is a free-function sister to [`Widget::render`] that additionally
/// accepts an optional [`MouseCtx`] for region recording. Each button in
/// `state.options` becomes one left-click region at `z_index = 1` (modal
/// layer). The button rect spans the `[<key>] <label>` text only — clicks
/// outside any button are no-ops.
///
/// Passing `None` produces output identical to calling
/// `frame.render_widget(dialog, area)`.
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    view: ConfirmDialog<'_>,
    mut ctx: Option<&mut MouseCtx<'_>>,
) {
    let state = view.state;

    // Fixed modal size (same as Widget::render).
    let modal_width = 50;
    let modal_height = 9;
    let modal_area = centered_rect(modal_width, modal_height, area);

    // Clear & block (same as Widget::render).
    Clear.render(modal_area, buf);
    let block = Block::default()
        .title(format!(" {} ", state.title))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_set(symbols::border::ROUNDED)
        .style(Style::default().bg(palette::POPUP_BG));
    let inner = block.inner(modal_area);
    block.render(modal_area, buf);

    // Layout (same as Widget::render).
    let chunks = Layout::vertical([
        Constraint::Length(1), // Spacer
        Constraint::Length(1), // Message line 1
        Constraint::Length(1), // Message line 2
        Constraint::Length(1), // Spacer
        Constraint::Length(1), // Buttons
        Constraint::Min(0),    // Rest
    ])
    .split(inner);

    // Message line (same as Widget::render).
    Paragraph::new(state.message.as_str())
        .alignment(Alignment::Center)
        .style(Style::default().fg(palette::STATUS_YELLOW))
        .render(chunks[1], buf);

    // Warning line (same as Widget::render).
    Paragraph::new("All Flutter processes will be terminated.")
        .alignment(Alignment::Center)
        .style(Style::default().fg(palette::TEXT_PRIMARY))
        .render(chunks[2], buf);

    // ── Buttons row with per-button click regions ──────────────────────────
    //
    // Build spans and per-button rects from `state.options`. The full button
    // text is centered within `chunks[4]` using the same formula ratatui's
    // `Alignment::Center` would apply. We compute `start_x` manually so that
    // the rect math for region recording matches the rendered positions.

    let button_row = chunks[4];

    // Colours per button index: green (confirm), red (cancel), yellow (tertiary).
    let key_colors = [
        palette::STATUS_GREEN,
        palette::STATUS_RED,
        palette::STATUS_YELLOW,
    ];

    // Build (label_str, key_char) pairs from options.
    // label_str is the full rendered segment: "[<key>] <Label>"
    // key_char is the first character of the label (lowercased).
    let segments: Vec<(String, char)> = state
        .options
        .iter()
        .map(|(label, _)| {
            let key = first_char_lower(label.as_str());
            (format!("[{}] {}", key, label), key)
        })
        .collect();

    // Two spaces between consecutive buttons (matches Widget::render's "  " gap).
    let separator = "  ";
    let total_width: usize = segments
        .iter()
        .map(|(s, _)| s.chars().count())
        .sum::<usize>()
        + separator.len() * segments.len().saturating_sub(1);

    // Compute the x coordinate where the first button starts (centering formula
    // that matches ratatui's Alignment::Center for a single line).
    let start_x =
        button_row.x + ((button_row.width as usize).saturating_sub(total_width) / 2) as u16;

    let last_idx = segments.len().saturating_sub(1);
    let mut render_spans: Vec<Span<'_>> = Vec::with_capacity(segments.len() * 4);
    let mut x = start_x;

    for (i, (segment, _key)) in segments.iter().enumerate() {
        let is_last = i == last_idx;
        let seg_width = segment.chars().count() as u16;
        let key_color = key_colors.get(i).copied().unwrap_or(palette::STATUS_YELLOW);

        // Build styled spans for "[<key>] <Label>".
        // segment format: "[k] Label" — chars: '[', key_char, ']', ' ', label...
        // The key character is at index 1.
        let key_ch = segment.chars().nth(1).unwrap_or(' ');
        let key_str: String = key_ch.to_string();
        // The BORDER_DIM span holds "] Label" and (for non-last buttons) the
        // 2-space separator. This matches the existing Widget::render exactly:
        //   - "] Yes  " for non-last (trailing spaces are part of BORDER_DIM span)
        //   - "] No"   for last (no trailing spaces)
        let after_key: String = {
            let base: String = segment.chars().skip(2).collect(); // "] Label"
            if is_last {
                base
            } else {
                format!("{}  ", base) // "] Label  " — 2-space gap in BORDER_DIM
            }
        };

        render_spans.push(Span::styled("[", Style::default().fg(palette::BORDER_DIM)));
        render_spans.push(Span::styled(
            key_str,
            Style::default().fg(key_color).add_modifier(Modifier::BOLD),
        ));
        render_spans.push(Span::styled(
            after_key,
            Style::default().fg(palette::BORDER_DIM),
        ));

        // Register a left-click region at z_index = 1 (modal layer).
        // The rect covers "[<key>] <Label>" only — the trailing 2-space separator
        // is excluded (a click on the gap between buttons is a no-op).
        if let Some(ref mut c) = ctx {
            let rect = MouseRect::new(x, button_row.y, seg_width, 1);
            if !rect.is_empty() {
                let msg = state.options[i].1.clone();
                c.click_at_z(rect, MouseAction::emit(msg), 1);
            }
        }

        x += seg_width;
        if !is_last {
            x += separator.len() as u16;
        }
    }

    // Render using Alignment::Center — identical to Widget::render — so that
    // the visual output is byte-identical regardless of the rect-math path.
    Paragraph::new(Line::from(render_spans))
        .alignment(Alignment::Center)
        .render(button_row, buf);
}

/// Lowercase the first character of `label`. `Yes` → `y`, `No` → `n`.
fn first_char_lower(label: &str) -> char {
    label
        .chars()
        .next()
        .map(|c| c.to_ascii_lowercase())
        .unwrap_or(' ')
}

impl Widget for ConfirmDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fixed modal size
        let modal_width = 50;
        let modal_height = 9;
        let modal_area = centered_rect(modal_width, modal_height, area);

        // Clear the area behind the modal
        Clear.render(modal_area, buf);

        // Create the modal block with border
        let block = Block::default()
            .title(format!(" {} ", self.state.title))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .style(Style::default().bg(palette::POPUP_BG));

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        // Layout: message + buttons
        let chunks = Layout::vertical([
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Message line 1
            Constraint::Length(1), // Message line 2
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Buttons
            Constraint::Min(0),    // Rest
        ])
        .split(inner);

        // Session count message
        let message = Paragraph::new(self.state.message.as_str())
            .alignment(Alignment::Center)
            .style(Style::default().fg(palette::STATUS_YELLOW));
        message.render(chunks[1], buf);

        // Warning message
        let warning = Paragraph::new("All Flutter processes will be terminated.")
            .alignment(Alignment::Center)
            .style(Style::default().fg(palette::TEXT_PRIMARY));
        warning.render(chunks[2], buf);

        // Buttons
        let buttons = Line::from(vec![
            Span::styled("[", Style::default().fg(palette::BORDER_DIM)),
            Span::styled(
                "y",
                Style::default()
                    .fg(palette::STATUS_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("] Yes  ", Style::default().fg(palette::BORDER_DIM)),
            Span::styled("[", Style::default().fg(palette::BORDER_DIM)),
            Span::styled(
                "n",
                Style::default()
                    .fg(palette::STATUS_RED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("] No", Style::default().fg(palette::BORDER_DIM)),
        ]);

        let buttons_para = Paragraph::new(buttons).alignment(Alignment::Center);
        buttons_para.render(chunks[4], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestTerminal;
    use fdemon_app::message::Message;
    use ratatui::{backend::TestBackend, Terminal};

    fn create_quit_dialog() -> ConfirmDialogState {
        ConfirmDialogState::new(
            "Quit?",
            "Are you sure you want to quit?",
            vec![("Yes", Message::ConfirmQuit), ("No", Message::CancelQuit)],
        )
    }

    fn create_close_session_dialog() -> ConfirmDialogState {
        ConfirmDialogState::new(
            "Close Session",
            "Close the current session?",
            vec![("Yes", Message::ConfirmQuit), ("No", Message::CancelQuit)],
        )
    }

    #[test]
    fn test_confirm_dialog_renders_title() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        assert!(term.buffer_contains("Quit"), "Dialog should show title");
    }

    #[test]
    fn test_confirm_dialog_renders_message() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        assert!(
            term.buffer_contains("sure") || term.buffer_contains("quit"),
            "Dialog should show confirmation message"
        );
    }

    #[test]
    fn test_confirm_dialog_shows_options() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Should show Yes/No or y/n options
        assert!(
            term.buffer_contains("Yes")
                || term.buffer_contains("y")
                || term.buffer_contains("No")
                || term.buffer_contains("n"),
            "Dialog should show confirmation options"
        );
    }

    #[test]
    fn test_confirm_dialog_shows_keybindings() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Should show key hints
        let content = term.content();
        assert!(
            content.contains("y")
                || content.contains("n")
                || content.contains("Enter")
                || content.contains("Esc"),
            "Dialog should show keybinding hints"
        );
    }

    #[test]
    fn test_confirm_dialog_different_actions() {
        let mut term = TestTerminal::new();

        // Quit dialog
        let quit_state = create_quit_dialog();
        let quit_dialog = ConfirmDialog::new(&quit_state);
        term.render_widget(quit_dialog, term.area());
        assert!(term.buffer_contains("Quit"));

        term.clear();

        // Close session dialog
        let close_state = create_close_session_dialog();
        let close_dialog = ConfirmDialog::new(&close_state);
        term.render_widget(close_dialog, term.area());
        assert!(term.buffer_contains("Close") || term.buffer_contains("Session"));
    }

    #[test]
    fn test_confirm_dialog_modal_overlay() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Modal should render (just verify no panic)
        let content = term.content();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_confirm_dialog_compact() {
        let mut term = TestTerminal::compact();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Should fit in small terminal
        let content = term.content();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_confirm_dialog_centered() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Dialog content should be roughly centered
        // (This is hard to verify precisely, just check it renders)
        let content = term.content();
        assert!(!content.is_empty());
    }

    // Legacy tests retained for backward compatibility
    #[test]
    fn test_confirm_dialog_state_single_session() {
        let state = ConfirmDialogState::quit_confirmation(1);
        assert!(state.message.contains("1 running session"));
        assert!(!state.message.contains("sessions"));
    }

    #[test]
    fn test_confirm_dialog_state_multiple_sessions() {
        let state = ConfirmDialogState::quit_confirmation(3);
        assert!(state.message.contains("3 running sessions"));
    }

    #[test]
    fn test_confirm_dialog_rendering() {
        let state = ConfirmDialogState::quit_confirmation(2);
        let dialog = ConfirmDialog::new(&state);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();

        // Should contain dialog elements
        assert!(content.contains("Quit"));
        assert!(content.contains("2 running sessions"));
        assert!(content.contains("y"));
        assert!(content.contains("n"));
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let modal = centered_rect(40, 10, area);

        // Should be centered
        assert_eq!(modal.x, 30); // (100 - 40) / 2
        assert_eq!(modal.y, 20); // (50 - 10) / 2
        assert_eq!(modal.width, 40);
        assert_eq!(modal.height, 10);
    }

    #[test]
    fn test_centered_rect_small_area() {
        let area = Rect::new(0, 0, 30, 8);
        let modal = centered_rect(50, 10, area);

        // Should be clamped to area
        assert_eq!(modal.width, 30);
        assert_eq!(modal.height, 8);
    }

    // ── render_with_regions tests ────────────────────────────────────────────

    #[test]
    fn render_with_regions_records_one_region_per_action_at_z1() {
        use crate::render::MouseCtx;
        use fdemon_app::{MouseRegions, MouseRegionsBuilder};

        let state = ConfirmDialogState::new(
            "Quit?",
            "Are you sure?",
            vec![("Yes", Message::ConfirmQuit), ("No", Message::CancelQuit)],
        );
        let dialog = ConfirmDialog::new(&state);

        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let mut regions = MouseRegions::default();
        {
            let builder: MouseRegionsBuilder<'_> = regions.builder();
            let mut ctx = MouseCtx::new(builder);
            super::render_with_regions(area, &mut buf, dialog, Some(&mut ctx));
        }

        assert_eq!(regions.len(), 2, "expected 2 button regions");

        for entry in regions.iter() {
            assert_eq!(entry.z_index, 1, "modal regions register at z=1");
            assert_eq!(entry.rect.height, 1);
            assert!(entry.rect.width > 0);
        }
    }

    #[test]
    fn render_with_regions_three_buttons_records_three_regions() {
        use crate::render::MouseCtx;
        use fdemon_app::{message::Message, MouseRegions, MouseRegionsBuilder};

        let state = ConfirmDialogState::new(
            "Unsaved changes",
            "What do you want to do?",
            vec![
                ("Save", Message::SettingsSaveAndClose),
                ("Discard", Message::ForceHideSettings),
                ("Cancel", Message::CancelQuit),
            ],
        );
        let dialog = ConfirmDialog::new(&state);

        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let mut regions = MouseRegions::default();
        {
            let builder: MouseRegionsBuilder<'_> = regions.builder();
            let mut ctx = MouseCtx::new(builder);
            super::render_with_regions(area, &mut buf, dialog, Some(&mut ctx));
        }
        assert_eq!(regions.len(), 3);
    }

    #[test]
    fn render_with_regions_visual_output_matches_widget_render() {
        // Render via Widget::render and via render_with_regions; assert pixel parity.
        let state = ConfirmDialogState::new(
            "Quit?",
            "Are you sure?",
            vec![("Yes", Message::ConfirmQuit), ("No", Message::CancelQuit)],
        );

        let area = Rect::new(0, 0, 80, 24);
        let mut buf_widget = Buffer::empty(area);
        let mut buf_with_regions = Buffer::empty(area);

        let dialog_a = ConfirmDialog::new(&state);
        Widget::render(dialog_a, area, &mut buf_widget);

        let dialog_b = ConfirmDialog::new(&state);
        super::render_with_regions(area, &mut buf_with_regions, dialog_b, None);

        assert_eq!(buf_widget, buf_with_regions, "visual output must match");
    }

    #[test]
    fn render_with_regions_none_ctx_produces_no_regions() {
        // Passing None for ctx must not panic and must produce no regions.
        let state = ConfirmDialogState::new(
            "Quit?",
            "Are you sure?",
            vec![("Yes", Message::ConfirmQuit), ("No", Message::CancelQuit)],
        );
        let dialog = ConfirmDialog::new(&state);
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);

        // Should not panic; region recording is skipped.
        super::render_with_regions(area, &mut buf, dialog, None);
        // The buffer should be non-empty (dialog rendered).
        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Quit"));
    }
}
