//! Shared TUI text helpers for performance details tab rendering.
//!
//! Provides three string-formatting utilities used by multiple sibling tabs in
//! the performance details pane:
//!
//! - [`truncate_with_ellipsis`] — truncate a string to a maximum Unicode scalar
//!   count, appending `…` when the string is truncated.
//! - [`pad_right`] — right-pad a string with spaces to a target display width.
//! - [`pad_left`] — left-pad a string with spaces to a target display width.
//!
//! All helpers operate on Unicode scalar values (chars), not bytes. For columns
//! that may contain wide grapheme clusters (e.g. CJK or emoji), callers should
//! use the `unicode-width` crate for accurate column accounting; these helpers
//! use `char::count()` semantics and document that limitation.

/// Total line count for the disabled/empty placeholder block.
/// Derived from: header line + spacer + hint line = 3.
pub(super) const PLACEHOLDER_LINE_COUNT: u16 = 3;

/// Truncate `s` to `max_chars` Unicode scalar values, appending `…` if
/// truncation occurs.
///
/// If `s` is at most `max_chars` characters long it is returned unchanged.
/// If `max_chars` is 0 and `s` is non-empty, returns `"…"` (the ellipsis alone).
///
/// # Unicode semantics
/// This helper counts Unicode scalar values (Rust `char`s), not grapheme
/// clusters or display columns. Callers rendering text in fixed-width terminal
/// columns should account for wide characters (e.g. CJK, emoji) separately.
pub(super) fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_owned()
    } else {
        let truncated: String = chars[..max_chars.saturating_sub(1)].iter().collect();
        format!("{truncated}…")
    }
}

/// Right-pad `s` with spaces to exactly `width` character positions.
///
/// If `s` is already at least `width` characters wide it is returned unchanged
/// (no truncation — callers should pre-truncate with
/// [`truncate_with_ellipsis`] if needed).
///
/// # Unicode semantics
/// Width is measured in Unicode scalar values, not display columns. Wide
/// characters count as 1.
pub(super) fn pad_right(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        s.to_owned()
    } else {
        format!("{}{}", s, " ".repeat(width - len))
    }
}

/// Left-pad `s` with spaces to exactly `width` character positions.
///
/// If `s` is already at least `width` characters wide it is returned unchanged.
///
/// # Unicode semantics
/// Width is measured in Unicode scalar values, not display columns. Wide
/// characters count as 1.
pub(super) fn pad_left(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        s.to_owned()
    } else {
        format!("{}{}", " ".repeat(width - len), s)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── truncate_with_ellipsis ────────────────────────────────────────────────

    #[test]
    fn truncate_empty_input_returns_empty() {
        assert_eq!(truncate_with_ellipsis("", 5), "");
    }

    #[test]
    fn truncate_zero_max_returns_ellipsis() {
        // max_chars = 0: chars[..0] = "" then appends '…' → "…"
        assert_eq!(truncate_with_ellipsis("Hello", 0), "…");
    }

    #[test]
    fn truncate_exact_fit_unchanged() {
        assert_eq!(truncate_with_ellipsis("Hello", 5), "Hello");
    }

    #[test]
    fn truncate_shorter_than_max_unchanged() {
        assert_eq!(truncate_with_ellipsis("Hi", 10), "Hi");
    }

    #[test]
    fn truncate_longer_than_max_appends_ellipsis() {
        let result = truncate_with_ellipsis("Hello World", 7);
        // 7 chars total: 6 chars + ellipsis
        assert_eq!(result.chars().count(), 7);
        assert!(
            result.ends_with('…'),
            "expected trailing '…', got: {result:?}"
        );
    }

    #[test]
    fn truncate_max_one_returns_ellipsis_only() {
        // max_chars = 1 → saturating_sub(1) = 0 chars + '…'
        let result = truncate_with_ellipsis("Hello", 1);
        assert_eq!(result, "…");
    }

    #[test]
    fn truncate_unicode_scalar_values_ascii_equivalent() {
        // ASCII-only: each char = 1 scalar.
        let result = truncate_with_ellipsis("abcdef", 4);
        assert_eq!(result.chars().count(), 4);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn truncate_emoji_single_scalar_counted_as_one() {
        // "🔥🔥🔥🔥" — 4 emoji chars (4 scalars)
        let s = "🔥🔥🔥🔥";
        // At max_chars = 4 (exact fit) — unchanged
        assert_eq!(truncate_with_ellipsis(s, 4), s);
        // At max_chars = 3 — truncate to 2 emoji + ellipsis
        let result = truncate_with_ellipsis(s, 3);
        assert_eq!(result.chars().count(), 3);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn truncate_wide_grapheme_cluster_cjk() {
        // CJK characters: each is a single scalar value.
        // "你好世界" = 4 scalars.
        let s = "你好世界";
        assert_eq!(truncate_with_ellipsis(s, 4), s); // exact fit
        let result = truncate_with_ellipsis(s, 3);
        assert_eq!(result.chars().count(), 3);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn truncate_mixed_ascii_and_unicode() {
        // "Hello 🌍" = 7 chars (6 ASCII + emoji)
        let s = "Hello 🌍";
        assert_eq!(truncate_with_ellipsis(s, 7), s); // exact fit
        let result = truncate_with_ellipsis(s, 6);
        assert_eq!(result.chars().count(), 6);
        assert!(result.ends_with('…'));
    }

    // ── pad_right ─────────────────────────────────────────────────────────────

    #[test]
    fn pad_right_empty_input_pads_to_width() {
        let result = pad_right("", 4);
        assert_eq!(result, "    ");
    }

    #[test]
    fn pad_right_zero_width_returns_string_unchanged() {
        assert_eq!(pad_right("Hi", 0), "Hi");
    }

    #[test]
    fn pad_right_exact_width_unchanged() {
        assert_eq!(pad_right("Hello", 5), "Hello");
    }

    #[test]
    fn pad_right_shorter_string_padded_with_spaces() {
        let result = pad_right("Hi", 5);
        assert_eq!(result, "Hi   ");
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn pad_right_longer_than_width_unchanged() {
        // No truncation — caller must pre-truncate.
        let result = pad_right("Hello World", 5);
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn pad_right_unicode_emoji_counts_as_one_char() {
        // "A🔥" = 2 chars; pad to width 5 → 3 trailing spaces
        let result = pad_right("A🔥", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("   "));
    }

    // ── pad_left ──────────────────────────────────────────────────────────────

    #[test]
    fn pad_left_empty_input_pads_to_width() {
        let result = pad_left("", 3);
        assert_eq!(result, "   ");
    }

    #[test]
    fn pad_left_zero_width_returns_string_unchanged() {
        assert_eq!(pad_left("Hi", 0), "Hi");
    }

    #[test]
    fn pad_left_exact_width_unchanged() {
        assert_eq!(pad_left("Hello", 5), "Hello");
    }

    #[test]
    fn pad_left_shorter_string_padded_on_left() {
        let result = pad_left("42", 5);
        assert_eq!(result, "   42");
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn pad_left_longer_than_width_unchanged() {
        let result = pad_left("Hello World", 5);
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn pad_left_unicode_emoji_counts_as_one_char() {
        // "🔥Z" = 2 chars; pad_left to width 5 → 3 leading spaces
        let result = pad_left("🔥Z", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.starts_with("   "));
    }
}
