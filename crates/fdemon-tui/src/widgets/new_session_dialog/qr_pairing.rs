//! Pair QR tab panel: QR code display + pairing status.
//!
//! Renders the ADB wireless-debugging pairing QR code (scannable straight off
//! the terminal) with phase-aware status text underneath. The pairing flow
//! itself runs in a background task; this widget is a pure projection of
//! [`QrPairingState`].

use qrcode::QrCode;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};
use tui_qrcode::{QrCodeWidget, QuietZone};

use crate::theme::palette;
use fdemon_app::new_session_dialog::{QrPairingPhase, QrPairingState};

/// Quiet-zone border added by [`QuietZone::Enabled`]: 4 modules on each side.
const QUIET_ZONE_MODULES: u16 = 8;

/// The Pair QR panel (content area of the Pair QR tab).
pub struct QrPairingPanel<'a> {
    pairing: Option<&'a QrPairingState>,
    /// Whether `adb` was found on PATH; without it pairing cannot run.
    adb_available: bool,
    /// Global animation frame for the status spinner.
    animation_frame: u64,
}

impl<'a> QrPairingPanel<'a> {
    pub fn new(pairing: Option<&'a QrPairingState>, adb_available: bool) -> Self {
        Self {
            pairing,
            adb_available,
            animation_frame: 0,
        }
    }

    /// Set the global animation frame used to drive the status spinner.
    pub fn animation_frame(mut self, frame: u64) -> Self {
        self.animation_frame = frame;
        self
    }
}

impl Widget for QrPairingPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if !self.adb_available {
            render_centered_message(
                area,
                buf,
                "adb not found on PATH.\n\
                 Install Android platform-tools to pair devices over Wi-Fi.",
                palette::STATUS_RED,
            );
            return;
        }

        let Some(pairing) = self.pairing else {
            render_centered_message(area, buf, "Starting QR pairing...", palette::TEXT_SECONDARY);
            return;
        };

        // Failed sessions show the error instead of a stale QR code.
        if let QrPairingPhase::Failed { error } = &pairing.phase {
            render_centered_message(
                area,
                buf,
                &format!("Pairing failed: {error}\n\nPress r to generate a new code."),
                palette::STATUS_RED,
            );
            return;
        }

        let Ok(qr) = QrCode::new(pairing.payload.as_bytes()) else {
            // Payload is generated internally; encoding cannot realistically
            // fail, but never panic in a render path.
            render_centered_message(area, buf, "Failed to encode QR code", palette::STATUS_RED);
            return;
        };

        // One terminal cell per module horizontally, half-block packing
        // vertically (2 modules per row).
        let qr_cols = qr.width() as u16 + QUIET_ZONE_MODULES;
        let qr_rows = qr_cols.div_ceil(2);

        // Layout: instruction line, QR code, status line.
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(qr_rows),
            Constraint::Length(1),
        ])
        .flex(ratatui::layout::Flex::Center)
        .split(area);

        if area.width < qr_cols || area.height < qr_rows + 2 {
            render_centered_message(
                area,
                buf,
                &format!(
                    "Terminal too small to display the pairing QR code\n\
                     (need {qr_cols}x{} here, have {}x{}).\n\
                     Resize the window and press r.",
                    qr_rows + 2,
                    area.width,
                    area.height
                ),
                palette::STATUS_YELLOW,
            );
            return;
        }

        let instruction = Paragraph::new(
            "Scan on your phone: Developer options › Wireless debugging › Pair device with QR code",
        )
        .style(Style::default().fg(palette::TEXT_SECONDARY))
        .alignment(Alignment::Center);
        instruction.render(chunks[0], buf);

        // Center the QR horizontally; the widget fills the given Rect, so hand
        // it exactly the cells it needs.
        let qr_area = center_horizontal(chunks[1], qr_cols);
        // Pin explicit black-on-white: the widget draws dark modules as
        // foreground glyphs, so theme defaults on a dark terminal would
        // produce an inverted QR that phone scanners often reject.
        QrCodeWidget::new(qr)
            .quiet_zone(QuietZone::Enabled)
            .style(
                Style::default()
                    .fg(ratatui::style::Color::Black)
                    .bg(ratatui::style::Color::White),
            )
            .render(qr_area, buf);

        let status = self.status_line(&pairing.phase);
        Paragraph::new(status)
            .alignment(Alignment::Center)
            .render(chunks[2], buf);
    }
}

impl QrPairingPanel<'_> {
    fn status_line(&self, phase: &QrPairingPhase) -> Line<'static> {
        let glyph = crate::widgets::spinner::spinner_char(
            self.animation_frame / crate::widgets::spinner::SPINNER_TICKS_PER_FRAME,
        );
        match phase {
            QrPairingPhase::WaitingForScan => Line::from(Span::styled(
                format!("{glyph} Waiting for scan...  (r generates a new code)"),
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
            // Failed is rendered as a full-panel message before reaching here.
            QrPairingPhase::Failed { error } => Line::from(Span::styled(
                format!("Pairing failed: {error}"),
                Style::default().fg(palette::STATUS_RED),
            )),
        }
    }
}

/// Center a `width`-column strip inside `area` (clamped to fit).
fn center_horizontal(area: Rect, width: u16) -> Rect {
    let width = width.min(area.width);
    let x = area.x + (area.width - width) / 2;
    Rect::new(x, area.y, width, area.height)
}

/// Render a vertically/horizontally centered multi-line message.
fn render_centered_message(
    area: Rect,
    buf: &mut Buffer,
    message: &str,
    color: ratatui::style::Color,
) {
    let line_count = message.lines().count() as u16;
    let chunks = Layout::vertical([Constraint::Length(line_count)])
        .flex(ratatui::layout::Flex::Center)
        .split(area);
    Paragraph::new(message)
        .style(Style::default().fg(color))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .render(chunks[0], buf);
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

    fn render_to_string(panel: QrPairingPanel<'_>, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| f.render_widget(panel, f.area())).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn renders_adb_missing_message() {
        let panel = QrPairingPanel::new(None, false);
        let content = render_to_string(panel, 60, 24);
        assert!(content.contains("adb not found"));
    }

    #[test]
    fn renders_starting_message_when_no_state() {
        let panel = QrPairingPanel::new(None, true);
        let content = render_to_string(panel, 60, 24);
        assert!(content.contains("Starting QR pairing"));
    }

    #[test]
    fn renders_qr_blocks_when_waiting() {
        let state = pairing_state(QrPairingPhase::WaitingForScan);
        let panel = QrPairingPanel::new(Some(&state), true);
        let content = render_to_string(panel, 80, 30);
        // Half-block QR rendering uses ▀/▄/█ glyphs.
        assert!(
            content.contains('█') || content.contains('▀') || content.contains('▄'),
            "expected QR half-block glyphs in output"
        );
        assert!(content.contains("Waiting for scan"));
        assert!(content.contains("Wireless debugging"));
    }

    #[test]
    fn renders_pairing_status_with_ip() {
        let state = pairing_state(QrPairingPhase::Pairing {
            ip: "192.168.1.42".to_string(),
        });
        let panel = QrPairingPanel::new(Some(&state), true);
        let content = render_to_string(panel, 80, 30);
        assert!(content.contains("192.168.1.42"));
        assert!(content.contains("pairing"));
    }

    #[test]
    fn renders_connecting_status() {
        let state = pairing_state(QrPairingPhase::Connecting {
            ip: "192.168.1.42".to_string(),
        });
        let panel = QrPairingPanel::new(Some(&state), true);
        let content = render_to_string(panel, 80, 30);
        assert!(content.contains("connecting to 192.168.1.42"));
    }

    #[test]
    fn renders_failure_message_without_qr() {
        let state = pairing_state(QrPairingPhase::Failed {
            error: "adb pair failed".to_string(),
        });
        let panel = QrPairingPanel::new(Some(&state), true);
        let content = render_to_string(panel, 80, 30);
        assert!(content.contains("Pairing failed: adb pair failed"));
        assert!(content.contains("Press r"));
        assert!(
            !content.contains('█'),
            "failed phase must not render the QR code"
        );
    }

    #[test]
    fn renders_too_small_hint_when_area_tiny() {
        let state = pairing_state(QrPairingPhase::WaitingForScan);
        let panel = QrPairingPanel::new(Some(&state), true);
        let content = render_to_string(panel, 20, 8);
        assert!(content.contains("too small"));
    }
}
