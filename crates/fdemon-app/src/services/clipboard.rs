//! Clipboard service — write text to the OS clipboard.
//!
//! This module provides the [`Clipboard`] trait with two implementations:
//!
//! - [`SystemClipboard`]: backed by [`arboard`], used by the runner at runtime.
//! - [`MemoryClipboard`]: records writes in-memory, used by unit tests that
//!   run headless without a display server.
//!
//! The trait is deliberately minimal — only [`Clipboard::write_text`] is
//! needed for the copy-to-clipboard feature. A `Send` bound is required
//! because the runner may construct the clipboard handle on one thread and
//! use it on the TEA dispatch thread.

use fdemon_core::Result;

/// Write text to the OS clipboard.
///
/// Implementors must be `Send` so the handle can be moved between threads.
pub trait Clipboard: Send {
    /// Write `text` to the clipboard, replacing any previous contents.
    ///
    /// # Errors
    ///
    /// Returns [`fdemon_core::Error::Terminal`] if the underlying clipboard
    /// implementation fails (e.g. no display server on the current platform).
    fn write_text(&mut self, text: &str) -> Result<()>;
}

// ─────────────────────────────────────────────────────────────────────────────
// SystemClipboard — backed by arboard
// ─────────────────────────────────────────────────────────────────────────────

/// OS clipboard backed by [`arboard::Clipboard`].
///
/// Construct once and keep alive for the duration of the session; `arboard`
/// may hold platform resources that are released on `Drop`.
pub struct SystemClipboard {
    inner: arboard::Clipboard,
}

impl SystemClipboard {
    /// Create a new [`SystemClipboard`], initialising the underlying
    /// platform clipboard handle.
    ///
    /// # Errors
    ///
    /// Returns [`fdemon_core::Error::Terminal`] if the clipboard cannot be
    /// initialised (e.g. no X11/Wayland display or Pasteboard service).
    pub fn new() -> Result<Self> {
        let inner = arboard::Clipboard::new()
            .map_err(|e| fdemon_core::Error::terminal(format!("clipboard init failed: {e}")))?;
        Ok(Self { inner })
    }
}

impl Clipboard for SystemClipboard {
    fn write_text(&mut self, text: &str) -> Result<()> {
        self.inner
            .set_text(text)
            .map_err(|e| fdemon_core::Error::terminal(format!("clipboard write failed: {e}")))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MemoryClipboard — in-memory stub for tests
// ─────────────────────────────────────────────────────────────────────────────

/// In-memory clipboard that records every write.
///
/// Use this in unit tests instead of [`SystemClipboard`]; it does not require
/// a display server and lets tests assert on the exact text that was written.
///
/// ```
/// use fdemon_app::services::{Clipboard, MemoryClipboard};
///
/// let mut cb = MemoryClipboard::default();
/// cb.write_text("hello").unwrap();
/// assert_eq!(cb.writes, vec!["hello"]);
/// ```
#[derive(Default)]
pub struct MemoryClipboard {
    /// All strings passed to [`Clipboard::write_text`], in call order.
    pub writes: Vec<String>,
}

impl Clipboard for MemoryClipboard {
    fn write_text(&mut self, text: &str) -> Result<()> {
        self.writes.push(text.to_string());
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_clipboard_records_writes() {
        let mut cb = MemoryClipboard::default();
        cb.write_text("first").unwrap();
        cb.write_text("second").unwrap();
        assert_eq!(cb.writes, vec!["first", "second"]);
    }

    #[test]
    fn test_memory_clipboard_returns_ok() {
        let mut cb = MemoryClipboard::default();
        let result = cb.write_text("anything");
        assert!(result.is_ok());
    }
}
