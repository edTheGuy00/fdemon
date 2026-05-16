//! Clipboard service — write text to the OS clipboard.
//!
//! This module provides the [`Clipboard`] trait with three implementations:
//!
//! - [`SystemClipboard`]: backed by [`arboard`], used by the runner at runtime.
//! - [`NullClipboard`]: returns an error on every write; used when the OS
//!   clipboard is unavailable (headless Linux, SSH without forwarding, etc.).
//! - [`MemoryClipboard`]: records writes in-memory, used by unit tests that
//!   run headless without a display server. **Only available in `#[cfg(test)]`.**
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
// NullClipboard — always-failing stub for unavailable OS clipboard
// ─────────────────────────────────────────────────────────────────────────────

/// Clipboard impl used when the OS clipboard is unavailable at runtime
/// (e.g. headless Linux without X/Wayland, ssh without forwarding,
/// sandboxed environment). Every write returns an error so the runner's
/// failure-toast path fires and the user sees that copy is non-functional.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullClipboard;

impl Clipboard for NullClipboard {
    fn write_text(&mut self, _text: &str) -> Result<()> {
        Err(fdemon_core::Error::terminal("system clipboard unavailable"))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MemoryClipboard — in-memory stub for tests (test-only)
// ─────────────────────────────────────────────────────────────────────────────

/// In-memory clipboard that records every write.
///
/// Use this in unit tests instead of [`SystemClipboard`]; it does not require
/// a display server and lets tests assert on the exact text that was written.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryClipboard {
    /// All strings passed to [`Clipboard::write_text`], in call order.
    pub writes: Vec<String>,
}

#[cfg(test)]
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

    #[test]
    fn test_null_clipboard_returns_err() {
        let mut c = NullClipboard;
        let result = c.write_text("hello");
        assert!(result.is_err(), "NullClipboard must return Err");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("system clipboard unavailable"),
            "error message must mention unavailability; got: {err_msg}"
        );
    }
}
