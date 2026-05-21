//! Braille-based plotting canvas for the memory chart.
//!
//! Each terminal character cell represents a 2x4 grid of braille dots,
//! providing 2x horizontal and 4x vertical sub-character resolution.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

// ── Braille canvas ────────────────────────────────────────────────────────────

/// Braille dot bit positions indexed by [y % 4][x % 2].
///
/// Unicode braille standard (U+2800–U+28FF):
///
/// ```text
/// Dot 1 (0x01) | Dot 4 (0x08)
/// Dot 2 (0x02) | Dot 5 (0x10)
/// Dot 3 (0x04) | Dot 6 (0x20)
/// Dot 7 (0x40) | Dot 8 (0x80)
/// ```
///
/// Each entry: BRAILLE_BIT_MAP[row_within_cell][col_within_cell]
pub(super) const BRAILLE_BIT_MAP: [[u8; 2]; 4] = [
    [0x01, 0x08], // row 0: dot 1, dot 4
    [0x02, 0x10], // row 1: dot 2, dot 5
    [0x04, 0x20], // row 2: dot 3, dot 6
    [0x40, 0x80], // row 3: dot 7, dot 8
];

/// A simple braille-based plotting canvas.
///
/// Each character cell is a 2x4 grid of dots (Unicode braille, U+2800–U+28FF).
/// This gives 2x horizontal and 4x vertical sub-character resolution.
///
/// Coordinates are in "dot space": x ranges 0..width*2, y ranges 0..height*4.
pub(super) struct BrailleCanvas {
    /// Braille dot-pattern offset per cell: cells[row][col].
    pub(super) cells: Vec<Vec<u8>>,
    /// Character columns.
    pub(super) width: usize,
    /// Character rows.
    pub(super) height: usize,
}

impl BrailleCanvas {
    /// Create a blank braille canvas with the given character dimensions.
    pub(super) fn new(width: usize, height: usize) -> Self {
        Self {
            cells: vec![vec![0u8; width]; height],
            width,
            height,
        }
    }

    /// Set a dot at `(x, y)` in dot-space coordinates.
    ///
    /// `x`: 0..width*2, `y`: 0..height*4.
    /// Out-of-bounds coordinates are silently ignored.
    pub(super) fn set(&mut self, x: usize, y: usize) {
        let col = x / 2;
        let row = y / 4;
        if col >= self.width || row >= self.height {
            return;
        }
        let bit = BRAILLE_BIT_MAP[y % 4][x % 2];
        self.cells[row][col] |= bit;
    }

    /// Render the canvas into a ratatui [`Buffer`] at the given position.
    ///
    /// Each non-empty cell is rendered as a Unicode braille character (U+2800 base
    /// + dot pattern). All cells in this canvas share the same `color` argument.
    ///
    /// Callers that draw multiple overlapping series should call this once per series
    /// with separate canvases.
    pub(super) fn render_to_buffer(&self, buf: &mut Buffer, area: Rect, color: Color) {
        let style = Style::default().fg(color);
        for row in 0..self.height {
            let y = area.y + row as u16;
            if y >= area.bottom() {
                break;
            }
            for col in 0..self.width {
                let x = area.x + col as u16;
                if x >= area.right() {
                    break;
                }
                let bits = self.cells[row][col];
                if bits != 0 {
                    let ch = char::from_u32(0x2800 + bits as u32).unwrap_or('\u{2800}');
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char(ch).set_style(style);
                    }
                }
            }
        }
    }
}
