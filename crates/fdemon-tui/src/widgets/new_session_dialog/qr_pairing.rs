//! ADB QR pairing modal: full-screen QR code display + pairing status.
//!
//! Opened with `p` from the New Session dialog. Renders over the entire
//! terminal so the QR code (which cannot shrink below one cell per module)
//! has enough space to be scannable in modest terminal sizes — a 39-char
//! payload needs a 29-module QR, i.e. 29 columns × 15 rows at half-block
//! packing, before quiet zone and chrome.
//!
//! The pairing flow itself runs in a background task; this widget is a pure
//! projection of [`QrPairingState`].

use qrcode::QrCode;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap},
};
use tui_qrcode::{QrCodeWidget, QuietZone};

use crate::theme::{palette, styles};
use crate::widgets::modal_overlay;
use fdemon_app::new_session_dialog::{QrPairingPhase, QrPairingState};

/// Horizontal white padding around the QR code, in cells (== modules).
/// Acts as the quiet zone; 4 modules matches the QR spec recommendation.
const QUIET_PAD_X: u16 = 4;

/// Minimum vertical white padding around the QR code, in rows. One row is
/// two modules at half-block packing; bumped to 2 rows (4 modules) when the
/// terminal is tall enough.
const QUIET_PAD_Y_MIN: u16 = 1;

/// Rows of chrome inside the modal besides the QR block:
/// 2 instruction lines + 1 blank + 1 status line.
const TEXT_ROWS: u16 = 4;

/// The ADB QR pairing modal.
pub struct QrPairingModal<'a> {
    state: &'a QrPairingState,
    /// Global animation frame for the status spinner.
    animation_frame: u64,
}

impl<'a> QrPairingModal<'a> {
    pub fn new(state: &'a QrPairingState) -> Self {
        Self {
            state,
            animation_frame: 0,
        }
    }

    /// Set the global animation frame used to drive the status spinner.
    pub fn animation_frame(mut self, frame: u64) -> Self {
        self.animation_frame = frame;
        self
    }
}

impl Widget for QrPairingModal<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        modal_overlay::dim_background(buf, area);

        // Failed sessions show the error instead of a stale QR code.
        if let QrPairingPhase::Failed { error } = &self.state.phase {
            self.render_message_modal(
                area,
                buf,
                &format!("Pairing failed:\n{error}"),
                palette::STATUS_RED,
            );
            return;
        }

        let Ok(qr) = QrCode::new(self.state.payload.as_bytes()) else {
            // Payload is generated internally; encoding cannot realistically
            // fail, but never panic in a render path.
            self.render_message_modal(area, buf, "Failed to encode QR code", palette::STATUS_RED);
            return;
        };

        // QR geometry: one cell per module horizontally, two modules per row
        // vertically (half blocks). QR codes are square, so width() is also
        // the module row count. The quiet zone is painted manually as a
        // white-filled pad around a QuietZone::Disabled render, so its
        // vertical cost can adapt to the terminal height.
        let modules = qr.width() as u16;
        let qr_rows = modules.div_ceil(2);
        let pad_y_extra_available = area
            .height
            .saturating_sub(2 + TEXT_ROWS + qr_rows + 2 * QUIET_PAD_Y_MIN + 2);
        let pad_y = if pad_y_extra_available >= 2 {
            QUIET_PAD_Y_MIN + 1
        } else {
            QUIET_PAD_Y_MIN
        };
        let qr_block_w = modules + 2 * QUIET_PAD_X;
        let qr_block_h = qr_rows + 2 * pad_y;

        // Modal: borders (2) + instructions (2) + blank (1) + QR block + status (1).
        let modal_h = 2 + TEXT_ROWS + qr_block_h;
        // Wide enough for the QR block and to keep the instruction text to
        // two lines; capped to the terminal.
        let modal_w = (qr_block_w + 4).max(60).min(area.width);

        if area.width < qr_block_w + 2 || area.height < modal_h {
            self.render_message_modal(
                area,
                buf,
                &format!(
                    "Terminal too small to display the pairing QR code.\n\
                     Need at least {}x{}, have {}x{}.\n\
                     Resize the window — the code stays valid.",
                    qr_block_w + 2,
                    modal_h,
                    area.width,
                    area.height
                ),
                palette::STATUS_YELLOW,
            );
            return;
        }

        let modal_area = center_rect(area, modal_w, modal_h);
        modal_overlay::render_shadow(buf, modal_area);
        modal_overlay::clear_area(buf, modal_area);

        let block = Block::default()
            .title(" Pair Android Device ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(styles::border_inactive())
            .style(Style::default().bg(palette::POPUP_BG));
        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        let chunks = Layout::vertical([
            Constraint::Length(2),          // Instructions
            Constraint::Length(1),          // Blank
            Constraint::Length(qr_block_h), // QR + quiet zone
            Constraint::Length(1),          // Status
        ])
        .split(inner);

        Paragraph::new(
            "On your Android device (11+), open Developer options\n\
             › Wireless debugging › Pair device with QR code, then scan:",
        )
        .style(Style::default().fg(palette::TEXT_SECONDARY))
        .alignment(Alignment::Center)
        .render(chunks[0], buf);

        // White quiet-zone pad with the QR centered inside. Explicit
        // black-on-white: the widget draws dark modules as foreground glyphs,
        // so theme defaults on a dark terminal would produce an inverted QR
        // that phone scanners often reject.
        let pad_area = center_rect(chunks[2], qr_block_w, qr_block_h);
        fill_white(pad_area, buf);
        let qr_area = Rect::new(
            pad_area.x + QUIET_PAD_X,
            pad_area.y + pad_y,
            modules,
            qr_rows,
        );
        QrCodeWidget::new(qr)
            .quiet_zone(QuietZone::Disabled)
            .style(Style::default().fg(Color::Black).bg(Color::White))
            .render(qr_area, buf);

        Paragraph::new(self.status_line())
            .alignment(Alignment::Center)
            .render(chunks[3], buf);
    }
}

impl QrPairingModal<'_> {
    fn status_line(&self) -> Line<'static> {
        let glyph = crate::widgets::spinner::spinner_char(
            self.animation_frame / crate::widgets::spinner::SPINNER_TICKS_PER_FRAME,
        );
        match &self.state.phase {
            QrPairingPhase::WaitingForScan => Line::from(Span::styled(
                format!("{glyph} Waiting for scan...   [r] New code  [Esc] Close"),
                Style::default().fg(palette::STATUS_YELLOW),
            )),
            QrPairingPhase::Pairing { ip } => Line::from(Span::styled(
                format!("{glyph} Phone found ({ip}) — pairing..."),
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            )),
            QrPairingPhase::Connecting { ip } => Line::from(Span::styled(
                format!("{glyph} Paired — connecting to {ip}..."),
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            )),
            // Failed is rendered as a dedicated message modal before reaching here.
            QrPairingPhase::Failed { error } => Line::from(Span::styled(
                format!("Pairing failed: {error}"),
                Style::default().fg(palette::STATUS_RED),
            )),
        }
    }

    /// Render a small centered message modal (errors, too-small fallback).
    fn render_message_modal(&self, area: Rect, buf: &mut Buffer, message: &str, color: Color) {
        let text_lines = message.lines().count() as u16;
        let modal_w = 56.min(area.width);
        // +1 footer hint line, +2 borders, +2 padding rows.
        let modal_h = (text_lines + 5).min(area.height);
        let modal_area = center_rect(area, modal_w, modal_h);

        modal_overlay::render_shadow(buf, modal_area);
        modal_overlay::clear_area(buf, modal_area);

        let block = Block::default()
            .title(" Pair Android Device ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(styles::border_inactive())
            .style(Style::default().bg(palette::POPUP_BG));
        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        let chunks = Layout::vertical([
            Constraint::Length(text_lines),
            Constraint::Length(1), // blank
            Constraint::Length(1), // hints
        ])
        .flex(Flex::Center)
        .split(inner);

        Paragraph::new(message)
            .style(Style::default().fg(color))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .render(chunks[0], buf);

        Paragraph::new("[r] New code  [Esc] Close")
            .style(Style::default().fg(palette::TEXT_MUTED))
            .alignment(Alignment::Center)
            .render(chunks[2], buf);
    }
}

/// Center a `w`×`h` rect inside `area` (clamped to fit).
fn center_rect(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

/// Fill a rect with white background cells (the QR quiet zone).
fn fill_white(area: Rect, buf: &mut Buffer) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            buf[(x, y)]
                .set_char(' ')
                .set_fg(Color::Black)
                .set_bg(Color::White);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::new_session_dialog::QrPairingState;
    use ratatui::{backend::TestBackend, Terminal};
    use tokio_util::sync::CancellationToken;

    fn pairing_state(phase: QrPairingPhase) -> QrPairingState {
        QrPairingState {
            seq: 0,
            payload: "WIFI:T:ADB;S:fdemon-123456;P:87654321;;".to_string(),
            phase,
            cancel: CancellationToken::new(),
        }
    }

    fn render_to_string(state: &QrPairingState, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| f.render_widget(QrPairingModal::new(state), f.area()))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn renders_qr_blocks_at_standard_80x24() {
        // The whole point of the modal approach: a standard 80x24 terminal
        // must show the actual QR code, not a too-small fallback.
        let state = pairing_state(QrPairingPhase::WaitingForScan);
        let content = render_to_string(&state, 80, 24);
        assert!(
            content.contains('█') || content.contains('▀') || content.contains('▄'),
            "expected QR half-block glyphs at 80x24"
        );
        assert!(!content.contains("too small"));
        assert!(content.contains("Waiting for scan"));
        assert!(content.contains("Wireless debugging"));
        assert!(content.contains("Pair device with QR code"));
    }

    #[test]
    fn renders_qr_blocks_at_large_size() {
        let state = pairing_state(QrPairingPhase::WaitingForScan);
        let content = render_to_string(&state, 120, 40);
        assert!(content.contains('█') || content.contains('▀') || content.contains('▄'));
    }

    #[test]
    fn renders_pairing_status_with_ip() {
        let state = pairing_state(QrPairingPhase::Pairing {
            ip: "192.168.1.42".to_string(),
        });
        let content = render_to_string(&state, 80, 24);
        assert!(content.contains("192.168.1.42"));
        assert!(content.contains("pairing"));
    }

    #[test]
    fn renders_connecting_status() {
        let state = pairing_state(QrPairingPhase::Connecting {
            ip: "192.168.1.42".to_string(),
        });
        let content = render_to_string(&state, 80, 24);
        assert!(content.contains("connecting to 192.168.1.42"));
    }

    #[test]
    fn renders_failure_message_without_qr() {
        let state = pairing_state(QrPairingPhase::Failed {
            error: "adb pair failed".to_string(),
        });
        let content = render_to_string(&state, 80, 24);
        assert!(content.contains("Pairing failed:"));
        assert!(content.contains("adb pair failed"));
        assert!(content.contains("[r] New code"));
        assert!(
            !content.contains('█'),
            "failed phase must not render the QR code"
        );
    }

    #[test]
    fn renders_too_small_hint_when_terminal_tiny() {
        let state = pairing_state(QrPairingPhase::WaitingForScan);
        let content = render_to_string(&state, 40, 12);
        assert!(content.contains("too small"));
    }

    #[test]
    fn qr_cells_are_black_on_white() {
        let state = pairing_state(QrPairingPhase::WaitingForScan);
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| f.render_widget(QrPairingModal::new(&state), f.area()))
            .unwrap();
        let buf = terminal.backend().buffer();
        let qr_cell = buf
            .content()
            .iter()
            .find(|c| c.symbol() == "█" || c.symbol() == "▀" || c.symbol() == "▄")
            .expect("QR glyph present");
        assert_eq!(qr_cell.fg, Color::Black);
        assert_eq!(qr_cell.bg, Color::White);
    }
}
