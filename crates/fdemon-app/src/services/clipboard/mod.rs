//! Clipboard service — write text to the user's clipboard.
//!
//! This module provides the [`Clipboard`] trait with these implementations:
//!
//! - [`SystemClipboard`]: backed by [`arboard`] (OS clipboard via
//!   X11/Wayland/macOS/Windows APIs).
//! - [`Osc52Clipboard`]: emits OSC 52 escape sequences to the terminal —
//!   the mechanism that works over SSH and on headless machines, where the
//!   terminal emulator on the user's local machine sets the clipboard.
//! - [`NullClipboard`]: returns an error on every write; used when no
//!   backend can work (or copy is disabled by config).
//! - [`MemoryClipboard`]: records writes in-memory, used by unit tests that
//!   run headless without a display server. **Only available in `#[cfg(test)]`.**
//!
//! Runners should not pick an implementation directly — [`create_clipboard`]
//! detects the environment (SSH session, multiplexer, display server, TTY)
//! and applies the configured [`ClipboardMode`] to choose the right backend.
//!
//! The trait is deliberately minimal — only [`Clipboard::write_text`] is
//! needed for the copy-to-clipboard feature. A `Send` bound is required
//! because the runner may construct the clipboard handle on one thread and
//! use it on the TEA dispatch thread.

pub mod detect;
pub mod osc52;

use tracing::{info, warn};

use fdemon_core::Result;

use crate::config::ClipboardMode;

use detect::{choose_backend, BackendChoice, ClipboardEnv};
use osc52::Osc52Clipboard;

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
// DisabledClipboard — copy explicitly turned off via config
// ─────────────────────────────────────────────────────────────────────────────

/// Clipboard impl used when the user set `ui.clipboard_mode = "off"`. Like
/// [`NullClipboard`] every write fails, but the message points at the config
/// option instead of claiming the OS clipboard is broken.
struct DisabledClipboard;

impl Clipboard for DisabledClipboard {
    fn write_text(&mut self, _text: &str) -> Result<()> {
        Err(fdemon_core::Error::terminal(
            "copy disabled (ui.clipboard_mode = off)",
        ))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Backend selection
// ─────────────────────────────────────────────────────────────────────────────

/// Outcome of clipboard backend selection at startup.
pub struct ClipboardInit {
    /// The chosen clipboard implementation.
    pub clipboard: Box<dyn Clipboard>,
    /// Backend name for logs: "system", "osc52", or "disabled".
    pub backend: &'static str,
    /// Set when copy cannot work in this environment; the runner surfaces it
    /// as a startup warning toast. `None` when copy is functional or was
    /// explicitly disabled by config.
    pub unavailable_reason: Option<String>,
}

/// Select and construct the clipboard backend for the current environment.
///
/// Applies the configured [`ClipboardMode`] to the detected environment (see
/// [`detect::choose_backend`] for the decision tree). In `Auto` mode an SSH
/// session gets OSC 52, a desktop session gets the OS clipboard, and a
/// headless terminal falls back to OSC 52 when the OS clipboard cannot
/// initialise.
pub fn create_clipboard(mode: ClipboardMode) -> ClipboardInit {
    let env = ClipboardEnv::from_process_env();
    let init = match choose_backend(mode, &env) {
        BackendChoice::Osc52(osc_mode) => ClipboardInit {
            clipboard: Box::new(Osc52Clipboard::new(osc_mode)),
            backend: "osc52",
            unavailable_reason: None,
        },
        BackendChoice::System { osc52_fallback } => match SystemClipboard::new() {
            Ok(cb) => ClipboardInit {
                clipboard: Box::new(cb),
                backend: "system",
                unavailable_reason: None,
            },
            Err(e) => {
                if let Some(osc_mode) = osc52_fallback {
                    info!("system clipboard unavailable ({e}); falling back to OSC 52");
                    ClipboardInit {
                        clipboard: Box::new(Osc52Clipboard::new(osc_mode)),
                        backend: "osc52",
                        unavailable_reason: None,
                    }
                } else {
                    warn!("system clipboard unavailable: {e}");
                    ClipboardInit {
                        clipboard: Box::new(NullClipboard),
                        backend: "disabled",
                        unavailable_reason: Some(e.to_string()),
                    }
                }
            }
        },
        BackendChoice::Disabled if mode == ClipboardMode::Off => ClipboardInit {
            clipboard: Box::new(DisabledClipboard),
            backend: "disabled",
            unavailable_reason: None,
        },
        BackendChoice::Disabled => ClipboardInit {
            clipboard: Box::new(NullClipboard),
            backend: "disabled",
            unavailable_reason: Some("no display server and stdout is not a terminal".to_string()),
        },
    };
    info!(
        "clipboard backend: {} (mode={mode}, ssh={}, screen={}, display={}, tty={})",
        init.backend, env.ssh, env.screen, env.display, env.stdout_is_tty
    );
    init
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

    #[test]
    fn test_disabled_clipboard_error_mentions_config_option() {
        let mut c = DisabledClipboard;
        let err_msg = format!("{}", c.write_text("hello").unwrap_err());
        assert!(
            err_msg.contains("ui.clipboard_mode"),
            "error must point at the config option; got: {err_msg}"
        );
    }

    #[test]
    fn test_create_clipboard_off_is_disabled_without_warning() {
        let init = create_clipboard(ClipboardMode::Off);
        assert_eq!(init.backend, "disabled");
        assert!(
            init.unavailable_reason.is_none(),
            "explicit off must not produce a startup warning"
        );
    }

    #[test]
    fn test_create_clipboard_forced_osc52_selects_osc52_backend() {
        // Forced OSC 52 never touches arboard or the display server, so this
        // is deterministic in headless test environments.
        let init = create_clipboard(ClipboardMode::Osc52);
        assert_eq!(init.backend, "osc52");
        assert!(init.unavailable_reason.is_none());
    }
}
