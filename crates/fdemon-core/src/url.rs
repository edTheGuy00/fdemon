//! URL utilities shared across crates.
//!
//! - [`percent_encode_uri`]: RFC 3986 query-parameter encoder.
//! - [`redact_devtools_url`]: strip the DDS auth-token path segment from a
//!   DevTools base URL for safe logging.

use std::fmt::Write as _;

/// Percent-encode a URI for use as a query parameter (RFC 3986).
///
/// Encodes all characters except the unreserved set (`A-Z`, `a-z`, `0-9`,
/// `-`, `_`, `.`, `~`). Uses uppercase hex digits per RFC 3986 §2.1.
///
/// This is intentionally a minimal byte-level percent-encoder (no `url`
/// crate dependency) — both the session layer and the DevTools handler
/// need it and we want them to agree byte-for-byte.
///
/// # Example
///
/// ```
/// # use fdemon_core::url::percent_encode_uri;
/// assert_eq!(
///     percent_encode_uri("ws://127.0.0.1:1234/abc=/ws"),
///     "ws%3A%2F%2F127.0.0.1%3A1234%2Fabc%3D%2Fws"
/// );
/// assert_eq!(percent_encode_uri("Aa0-_.~"), "Aa0-_.~");
/// ```
pub fn percent_encode_uri(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            // write! to String is infallible
            _ => {
                let _ = write!(encoded, "%{:02X}", byte);
            }
        }
    }
    encoded
}

/// Redact path segments that may contain a DDS authentication token so a
/// DevTools URL can be safely written to logs.
///
/// Flutter ≥ 3.24 DDS-integrated URLs embed an auth token as a URL path
/// segment, e.g. `http://127.0.0.1:59123/tbrR0DzW2j8=/devtools`. That
/// token is a bearer credential — anyone who reads it from a log file can
/// reach the VM Service. This function rewrites the path so the segments
/// between scheme://host and the final `/devtools` (or the end of the URL)
/// become `<REDACTED>`.
///
/// URLs without a path (e.g. `http://127.0.0.1:9100`) are returned
/// unchanged — there is no auth material to hide.
///
/// This is a best-effort textual transform, not a parser. If the input
/// is not a recognisable `http://`/`https://` URL it is returned verbatim.
///
/// # Examples
///
/// ```
/// # use fdemon_core::url::redact_devtools_url;
/// // Standalone DevTools — nothing to redact.
/// assert_eq!(
///     redact_devtools_url("http://127.0.0.1:9100"),
///     "http://127.0.0.1:9100"
/// );
///
/// // DDS-integrated — token is hidden.
/// assert_eq!(
///     redact_devtools_url("http://127.0.0.1:59123/tbrR0DzW2j8=/devtools"),
///     "http://127.0.0.1:59123/<REDACTED>/devtools"
/// );
/// ```
pub fn redact_devtools_url(url: &str) -> String {
    // Identify scheme + authority prefix.
    let scheme_end = if let Some(rest) = url.strip_prefix("http://") {
        url.len() - rest.len()
    } else if let Some(rest) = url.strip_prefix("https://") {
        url.len() - rest.len()
    } else {
        return url.to_string();
    };

    // Find the first '/' after the authority. If none, there is no path
    // (and therefore no auth token to redact).
    let path_start = match url[scheme_end..].find('/') {
        Some(rel) => scheme_end + rel,
        None => return url.to_string(),
    };
    let (authority, path) = url.split_at(path_start);

    // Preserve a trailing `/devtools` (or `/devtools/...`) suffix so log
    // readers can still tell it is a DevTools URL.
    let suffix = if let Some(idx) = path.rfind("/devtools") {
        &path[idx..]
    } else {
        ""
    };

    let mut out = String::with_capacity(authority.len() + suffix.len() + "/<REDACTED>".len());
    out.push_str(authority);
    out.push_str("/<REDACTED>");
    out.push_str(suffix);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreserved_passthrough() {
        assert_eq!(
            percent_encode_uri("ABCxyz0123-_.~"),
            "ABCxyz0123-_.~",
            "RFC 3986 unreserved set must pass through unchanged"
        );
    }

    #[test]
    fn encodes_reserved_characters() {
        assert_eq!(percent_encode_uri(":/?#[]@"), "%3A%2F%3F%23%5B%5D%40");
    }

    #[test]
    fn encodes_space_as_percent_20() {
        // Critically NOT '+' — '+' encoding is form-urlencoded, not RFC 3986.
        assert_eq!(percent_encode_uri("a b"), "a%20b");
    }

    #[test]
    fn encodes_utf8_multibyte() {
        // 'é' is U+00E9, UTF-8 bytes 0xC3 0xA9.
        assert_eq!(percent_encode_uri("é"), "%C3%A9");
    }

    #[test]
    fn typical_ws_uri() {
        assert_eq!(
            percent_encode_uri("ws://127.0.0.1:1234/abc=/ws"),
            "ws%3A%2F%2F127.0.0.1%3A1234%2Fabc%3D%2Fws"
        );
    }

    // -- redact_devtools_url -----------------------------------------------

    #[test]
    fn redact_passes_through_plain_url_without_path() {
        assert_eq!(
            redact_devtools_url("http://127.0.0.1:9100"),
            "http://127.0.0.1:9100"
        );
    }

    #[test]
    fn redact_hides_dds_auth_token() {
        assert_eq!(
            redact_devtools_url("http://127.0.0.1:59123/tbrR0DzW2j8=/devtools"),
            "http://127.0.0.1:59123/<REDACTED>/devtools"
        );
    }

    #[test]
    fn redact_hides_token_for_https() {
        assert_eq!(
            redact_devtools_url("https://localhost:8181/SECRET-TOKEN/devtools"),
            "https://localhost:8181/<REDACTED>/devtools"
        );
    }

    #[test]
    fn redact_preserves_trailing_devtools_path_suffix() {
        // /devtools/inspector (deeper path) should keep the suffix visible.
        assert_eq!(
            redact_devtools_url("http://127.0.0.1:59123/AUTH=/devtools/inspector"),
            "http://127.0.0.1:59123/<REDACTED>/devtools/inspector"
        );
    }

    #[test]
    fn redact_passes_through_non_http_input() {
        // Defense-in-depth: never panic, never crash. Unknown schemes are
        // returned verbatim — caller is responsible for not logging them.
        assert_eq!(redact_devtools_url("ws://example.com"), "ws://example.com");
        assert_eq!(redact_devtools_url(""), "");
    }

    #[test]
    fn redact_handles_token_without_devtools_suffix() {
        // Pathological case: a URL with a path but no /devtools tail. Still
        // redact — better safe than sorry.
        assert_eq!(
            redact_devtools_url("http://127.0.0.1:9100/MAYBE-A-TOKEN"),
            "http://127.0.0.1:9100/<REDACTED>"
        );
    }
}
