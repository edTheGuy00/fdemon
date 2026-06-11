//! OSC 52 clipboard backend.
//!
//! Emits the xterm `OSC 52` escape sequence (`ESC ] 52 ; c ; <base64> BEL`)
//! to the terminal, which sets the clipboard on the machine the terminal
//! emulator runs on. This is the standard mechanism for copy-to-clipboard
//! over SSH, where no display server (and therefore no OS clipboard) is
//! reachable from the remote process.
//!
//! Environment handling follows current (2025) practice in helix/neovim:
//!
//! - **Plain terminals and tmux** receive the raw sequence. tmux intercepts
//!   OSC 52 itself and forwards it to the outer terminal when
//!   `set-clipboard on` is configured; DCS passthrough wrapping is *not*
//!   used because tmux ≥ 3.3 ships `allow-passthrough off` by default,
//!   which would silently drop the wrapped sequence.
//! - **GNU screen** does not understand OSC 52 but passes DCS contents
//!   through unchanged, so the sequence is split into 76-byte chunks each
//!   wrapped in `ESC P … ESC \`.
//!
//! Payloads are capped at [`MAX_TEXT_BYTES`] (xterm accepts at most 100,000
//! bytes for the whole sequence); oversized text is truncated at a char
//! boundary with a warning log rather than failing the copy outright.

use std::io::Write;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use tracing::warn;

use fdemon_core::Result;

use super::Clipboard;

/// Maximum raw text bytes accepted before truncation.
///
/// Derived from xterm's 100,000-byte cap on the entire control sequence:
/// base64 expands 74,994 bytes to 99,992, leaving room for the 7-byte
/// header and 1-byte terminator (the derivation used by hterm's osc52.sh).
pub const MAX_TEXT_BYTES: usize = 74_994;

/// DCS chunk payload size used for GNU screen wrapping (matches go-osc52
/// and hterm; safe across screen versions).
const SCREEN_CHUNK_BYTES: usize = 76;

/// How the OSC 52 sequence must be framed for the active terminal stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Osc52Mode {
    /// Raw OSC 52 — plain terminals and tmux (which forwards it natively).
    Plain,
    /// DCS-wrapped 76-byte chunks for GNU screen.
    Screen,
}

/// Build the complete byte sequence that sets the terminal clipboard to
/// `text` (already truncated by the caller if oversized).
fn build_sequence(text: &str, mode: Osc52Mode) -> Vec<u8> {
    let osc = format!("\x1b]52;c;{}\x07", BASE64.encode(text.as_bytes()));
    match mode {
        Osc52Mode::Plain => osc.into_bytes(),
        Osc52Mode::Screen => {
            // Screen relays DCS contents verbatim to the outer terminal, but
            // caps DCS length — chunk the whole OSC sequence at 76 bytes,
            // each chunk framed as ESC P <chunk> ESC \.
            let mut out = Vec::with_capacity(osc.len() + (osc.len() / SCREEN_CHUNK_BYTES + 1) * 4);
            for chunk in osc.as_bytes().chunks(SCREEN_CHUNK_BYTES) {
                out.extend_from_slice(b"\x1bP");
                out.extend_from_slice(chunk);
                out.extend_from_slice(b"\x1b\\");
            }
            out
        }
    }
}

/// Truncate `text` to at most [`MAX_TEXT_BYTES`], backing up to a char
/// boundary. Returns the (possibly shortened) slice.
fn truncate_to_limit(text: &str) -> &str {
    if text.len() <= MAX_TEXT_BYTES {
        return text;
    }
    // Cannot underflow: byte 0 is always a char boundary (and a UTF-8 char
    // spans at most 4 bytes, so this backs up at most 3).
    let mut end = MAX_TEXT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Clipboard backed by OSC 52 escape sequences written to the terminal.
///
/// The sequence is written to stdout — the same fd the TUI renders to — and
/// flushed immediately. The runner invokes clipboard writes between frames
/// (never inside a `draw` closure), so the escape bytes cannot interleave
/// with ratatui's buffered frame output.
pub struct Osc52Clipboard<W: Write + Send = std::io::Stdout> {
    mode: Osc52Mode,
    sink: W,
}

impl Osc52Clipboard {
    /// Create an OSC 52 clipboard writing to stdout.
    pub fn new(mode: Osc52Mode) -> Self {
        Self {
            mode,
            sink: std::io::stdout(),
        }
    }
}

impl<W: Write + Send> Osc52Clipboard<W> {
    /// Create an OSC 52 clipboard writing to an arbitrary sink (tests).
    pub fn with_sink(mode: Osc52Mode, sink: W) -> Self {
        Self { mode, sink }
    }
}

impl<W: Write + Send> Clipboard for Osc52Clipboard<W> {
    fn write_text(&mut self, text: &str) -> Result<()> {
        let payload = truncate_to_limit(text);
        if payload.len() < text.len() {
            warn!(
                "OSC 52 payload truncated from {} to {} bytes (terminal sequence size limit)",
                text.len(),
                payload.len()
            );
        }
        let seq = build_sequence(payload, self.mode);
        self.sink
            .write_all(&seq)
            .and_then(|_| self.sink.flush())
            .map_err(|e| {
                fdemon_core::Error::terminal(format!("OSC 52 clipboard write failed: {e}"))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_with_mode(text: &str, mode: Osc52Mode) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut cb = Osc52Clipboard::with_sink(mode, &mut buf);
        cb.write_text(text).unwrap();
        buf
    }

    // ─── plain sequence format ───────────────────────────────────────────────

    #[test]
    fn test_plain_sequence_exact_bytes() {
        // base64("hello") == "aGVsbG8="
        let out = write_with_mode("hello", Osc52Mode::Plain);
        assert_eq!(out, b"\x1b]52;c;aGVsbG8=\x07");
    }

    #[test]
    fn test_plain_sequence_empty_text_clears_clipboard() {
        let out = write_with_mode("", Osc52Mode::Plain);
        assert_eq!(out, b"\x1b]52;c;\x07");
    }

    #[test]
    fn test_base64_uses_standard_alphabet_with_padding_no_newlines() {
        // "ÿÿ" (UTF-8: c3 bf c3 bf) encodes to "w7/Dvw==" — the '/' only
        // appears in the standard alphabet, '=' checks padding is kept.
        let out = write_with_mode("ÿÿ", Osc52Mode::Plain);
        let body = &out[7..out.len() - 1];
        assert_eq!(body, b"w7/Dvw==");
        assert!(!out.contains(&b'\n'), "payload must not contain newlines");
    }

    #[test]
    fn test_plain_sequence_multiline_text() {
        let out = write_with_mode("line1\nline2", Osc52Mode::Plain);
        let body = std::str::from_utf8(&out[7..out.len() - 1]).unwrap();
        let decoded = BASE64.decode(body).unwrap();
        assert_eq!(decoded, b"line1\nline2");
    }

    // ─── screen DCS chunking ─────────────────────────────────────────────────

    #[test]
    fn test_screen_short_sequence_single_dcs_chunk() {
        let out = write_with_mode("hi", Osc52Mode::Screen);
        // Whole OSC sequence fits in one 76-byte chunk.
        assert_eq!(out, b"\x1bP\x1b]52;c;aGk=\x07\x1b\\");
    }

    #[test]
    fn test_screen_long_sequence_chunked_at_76_bytes() {
        let text = "x".repeat(300);
        let out = write_with_mode(&text, Osc52Mode::Screen);

        // Reconstruct the inner OSC sequence by stripping DCS framing.
        let osc = format!("\x1b]52;c;{}\x07", BASE64.encode(text.as_bytes()));
        let expected_chunks = osc.as_bytes().chunks(76).count();

        let mut inner = Vec::new();
        let mut rest: &[u8] = &out;
        let mut chunks = 0;
        while !rest.is_empty() {
            assert_eq!(&rest[..2], b"\x1bP", "each chunk must open with DCS");
            let end = rest
                .windows(2)
                .position(|w| w == b"\x1b\\")
                .expect("each chunk must close with ST");
            inner.extend_from_slice(&rest[2..end]);
            rest = &rest[end + 2..];
            chunks += 1;
        }
        assert_eq!(chunks, expected_chunks);
        assert_eq!(
            inner,
            osc.as_bytes(),
            "concatenated chunks must rebuild the OSC sequence"
        );
        for chunk_len in inner.chunks(76).map(|c| c.len()) {
            assert!(chunk_len <= 76);
        }
    }

    // ─── truncation ──────────────────────────────────────────────────────────

    #[test]
    fn test_truncate_under_limit_is_noop() {
        let text = "a".repeat(MAX_TEXT_BYTES);
        assert_eq!(truncate_to_limit(&text).len(), MAX_TEXT_BYTES);
    }

    #[test]
    fn test_truncate_over_limit_caps_payload() {
        let text = "a".repeat(MAX_TEXT_BYTES + 1000);
        assert_eq!(truncate_to_limit(&text).len(), MAX_TEXT_BYTES);
    }

    #[test]
    fn test_truncate_respects_char_boundary() {
        // Fill so a 4-byte emoji straddles the limit.
        let mut text = "a".repeat(MAX_TEXT_BYTES - 2);
        text.push('🦀'); // 4 bytes, crosses MAX_TEXT_BYTES
        text.push_str(&"b".repeat(100));
        let cut = truncate_to_limit(&text);
        assert!(cut.len() <= MAX_TEXT_BYTES);
        assert_eq!(
            cut.len(),
            MAX_TEXT_BYTES - 2,
            "must back up to char boundary"
        );
        assert!(cut.is_char_boundary(cut.len()));
    }

    #[test]
    fn test_oversized_write_succeeds_with_truncated_payload() {
        let text = "a".repeat(MAX_TEXT_BYTES * 2);
        let out = write_with_mode(&text, Osc52Mode::Plain);
        let body = std::str::from_utf8(&out[7..out.len() - 1]).unwrap();
        let decoded = BASE64.decode(body).unwrap();
        assert_eq!(decoded.len(), MAX_TEXT_BYTES);
    }

    // ─── error propagation ───────────────────────────────────────────────────

    struct FailingSink;
    impl Write for FailingSink {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("sink closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_sink_error_maps_to_terminal_error() {
        let mut cb = Osc52Clipboard::with_sink(Osc52Mode::Plain, FailingSink);
        let err = cb.write_text("hello").unwrap_err();
        assert!(
            err.to_string().contains("OSC 52 clipboard write failed"),
            "got: {err}"
        );
    }
}
