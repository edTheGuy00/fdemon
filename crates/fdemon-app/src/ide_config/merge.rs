//! Shared JSON and TOML merge utilities for IDE config generators.
//!
//! Provides helpers for finding and replacing entries in JSON arrays,
//! cleaning JSONC (JSON with comments), and consistent pretty-printing.
//! These utilities are reused across per-IDE generator implementations.

/// Marker field name used to identify fdemon-managed entries in JSON configs.
///
/// Reserved for generators that write a boolean marker field instead of matching by name.
/// Currently unused in production code but retained as a named protocol constant.
#[allow(dead_code)]
pub(crate) const FDEMON_MARKER_FIELD: &str = "fdemon-managed";

/// Marker value for the fdemon config entry name field.
pub(crate) const FDEMON_CONFIG_NAME: &str = "Flutter (fdemon)";

/// Find an entry in a JSON array by a string field value.
///
/// Searches `array` for an object whose field named `field` has a string value
/// equal to `value`. Returns the index of the first matching entry, or `None`.
pub(crate) fn find_json_entry_by_field(
    array: &[serde_json::Value],
    field: &str,
    value: &str,
) -> Option<usize> {
    array.iter().position(|entry| {
        entry
            .get(field)
            .and_then(|v| v.as_str())
            .map(|s| s == value)
            .unwrap_or(false)
    })
}

/// Merge a new entry into a JSON array, replacing an existing entry
/// matched by `field == value`, or appending if not found.
///
/// If an existing entry in `array` has a field named `field` with string value
/// `value`, it is replaced in-place with `new_entry`. Otherwise `new_entry`
/// is appended to the end of `array`.
pub(crate) fn merge_json_array_entry(
    array: &mut Vec<serde_json::Value>,
    field: &str,
    value: &str,
    new_entry: serde_json::Value,
) {
    match find_json_entry_by_field(array, field, value) {
        Some(idx) => array[idx] = new_entry,
        None => array.push(new_entry),
    }
}

/// Clean JSONC (JSON with comments) to valid JSON.
///
/// VSCode and other editors use JSONC which allows:
/// - `//` line comments
/// - `/* */` block comments
/// - Trailing commas in arrays and objects
pub(crate) fn clean_jsonc(input: &str) -> String {
    let without_comments = strip_json_comments(input);
    strip_trailing_commas(&without_comments)
}

/// Serialize a JSON value with consistent pretty-printing (2-space indent).
pub(crate) fn to_pretty_json(value: &serde_json::Value) -> String {
    // serde_json's pretty-print uses 2-space indent by default
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

/// Strip `//` line comments and `/* */` block comments from JSON.
fn strip_json_comments(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            continue;
        }

        if c == '"' && !escape_next {
            in_string = !in_string;
            result.push(c);
            continue;
        }

        if !in_string && c == '/' {
            if let Some(&next) = chars.peek() {
                if next == '/' {
                    // Line comment — skip until newline
                    chars.next(); // consume second '/'
                    while let Some(&nc) = chars.peek() {
                        if nc == '\n' {
                            break;
                        }
                        chars.next();
                    }
                    continue;
                } else if next == '*' {
                    // Block comment — skip until '*/'
                    chars.next(); // consume '*'
                    while let Some(nc) = chars.next() {
                        if nc == '*' {
                            if let Some(&'/') = chars.peek() {
                                chars.next(); // consume '/'
                                break;
                            }
                        }
                    }
                    continue;
                }
            }
        }

        result.push(c);
    }

    result
}

/// Strip trailing commas before `]` and `}`.
///
/// Trailing commas are valid in JSONC but not in standard JSON.
/// Commas inside string literals are preserved unchanged.
fn strip_trailing_commas(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            continue;
        }

        if c == '"' {
            in_string = !in_string;
            result.push(c);
            continue;
        }

        if !in_string && c == ',' {
            // Peek ahead (skipping whitespace) to see if the comma is trailing
            let mut is_trailing = false;
            let mut peek_chars = chars.clone();

            while let Some(&next) = peek_chars.peek() {
                if next.is_whitespace() {
                    peek_chars.next();
                } else {
                    is_trailing = next == ']' || next == '}';
                    break;
                }
            }

            if !is_trailing {
                result.push(c);
            }
            // Trailing comma is silently dropped
            continue;
        }

        result.push(c);
    }

    result
}

// ─────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── find_json_entry_by_field ────────────────────────────────

    #[test]
    fn test_find_json_entry_by_field_found() {
        let array = vec![
            json!({"name": "Dart", "type": "dart"}),
            json!({"name": "Flutter (fdemon)", "type": "dart"}),
        ];
        assert_eq!(
            find_json_entry_by_field(&array, "name", "Flutter (fdemon)"),
            Some(1)
        );
    }

    #[test]
    fn test_find_json_entry_by_field_not_found() {
        let array = vec![json!({"name": "Dart"})];
        assert_eq!(
            find_json_entry_by_field(&array, "name", "Flutter (fdemon)"),
            None
        );
    }

    #[test]
    fn test_find_json_entry_by_field_first_match() {
        let array = vec![
            json!({"name": "Flutter (fdemon)", "port": 1}),
            json!({"name": "Flutter (fdemon)", "port": 2}),
        ];
        // Returns the index of the first match
        assert_eq!(
            find_json_entry_by_field(&array, "name", "Flutter (fdemon)"),
            Some(0)
        );
    }

    #[test]
    fn test_find_json_entry_by_field_empty_array() {
        let array: Vec<serde_json::Value> = vec![];
        assert_eq!(find_json_entry_by_field(&array, "name", "anything"), None);
    }

    #[test]
    fn test_find_json_entry_by_field_non_string_value() {
        // Entry whose field is a number — should not match
        let array = vec![json!({"name": 42})];
        assert_eq!(find_json_entry_by_field(&array, "name", "42"), None);
    }

    // ── merge_json_array_entry ──────────────────────────────────

    #[test]
    fn test_merge_json_array_entry_replaces_existing() {
        let mut array = vec![
            json!({"name": "existing"}),
            json!({"name": "Flutter (fdemon)", "debugServer": 1234}),
        ];
        merge_json_array_entry(
            &mut array,
            "name",
            "Flutter (fdemon)",
            json!({"name": "Flutter (fdemon)", "debugServer": 5678}),
        );
        assert_eq!(array.len(), 2);
        assert_eq!(array[1]["debugServer"], 5678);
    }

    #[test]
    fn test_merge_json_array_entry_appends_new() {
        let mut array = vec![json!({"name": "existing"})];
        merge_json_array_entry(
            &mut array,
            "name",
            "Flutter (fdemon)",
            json!({"name": "Flutter (fdemon)"}),
        );
        assert_eq!(array.len(), 2);
        assert_eq!(array[1]["name"], "Flutter (fdemon)");
    }

    #[test]
    fn test_merge_json_array_entry_appends_to_empty() {
        let mut array: Vec<serde_json::Value> = vec![];
        merge_json_array_entry(
            &mut array,
            "name",
            "Flutter (fdemon)",
            json!({"name": "Flutter (fdemon)"}),
        );
        assert_eq!(array.len(), 1);
    }

    #[test]
    fn test_merge_json_array_entry_preserves_others() {
        let mut array = vec![
            json!({"name": "Config A"}),
            json!({"name": "Flutter (fdemon)", "port": 1}),
            json!({"name": "Config B"}),
        ];
        merge_json_array_entry(
            &mut array,
            "name",
            "Flutter (fdemon)",
            json!({"name": "Flutter (fdemon)", "port": 9999}),
        );
        assert_eq!(array.len(), 3);
        assert_eq!(array[0]["name"], "Config A");
        assert_eq!(array[1]["port"], 9999);
        assert_eq!(array[2]["name"], "Config B");
    }

    // ── clean_jsonc ─────────────────────────────────────────────

    #[test]
    fn test_clean_jsonc_strips_line_comments() {
        assert_eq!(
            clean_jsonc("{\n  // comment\n  \"key\": 1\n}"),
            "{\n  \n  \"key\": 1\n}"
        );
    }

    #[test]
    fn test_clean_jsonc_strips_trailing_commas() {
        let input = r#"{"items": [1, 2,]}"#;
        let cleaned = clean_jsonc(input);
        let parsed: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn test_clean_jsonc_strips_block_comments() {
        let input = r#"{"key": /* comment */ "value"}"#;
        let cleaned = clean_jsonc(input);
        assert!(!cleaned.contains("comment"));
        let parsed: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn test_clean_jsonc_preserves_slashes_in_strings() {
        let input = r#"{"url": "https://example.com/path"}"#;
        let cleaned = clean_jsonc(input);
        assert_eq!(cleaned, input);
    }

    #[test]
    fn test_clean_jsonc_combined_comments_and_trailing_commas() {
        let input = r#"{
            // Comment
            "arr": [
                "value",
            ]
        }"#;
        let cleaned = clean_jsonc(input);
        assert!(!cleaned.contains("// Comment"));
        assert!(!cleaned.contains("\"value\","));
        let parsed: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert!(parsed.is_object());
    }

    // ── to_pretty_json ──────────────────────────────────────────

    #[test]
    fn test_to_pretty_json_produces_valid_json() {
        let value = json!({"key": "value", "num": 42});
        let pretty = to_pretty_json(&value);
        let reparsed: serde_json::Value = serde_json::from_str(&pretty).unwrap();
        assert_eq!(reparsed, value);
    }

    #[test]
    fn test_to_pretty_json_uses_indentation() {
        let value = json!({"nested": {"key": "val"}});
        let pretty = to_pretty_json(&value);
        // serde_json pretty-print uses 2-space indentation
        assert!(pretty.contains("  "));
    }

    // ── constants ───────────────────────────────────────────────

    #[test]
    fn test_constants_have_expected_values() {
        assert_eq!(FDEMON_MARKER_FIELD, "fdemon-managed");
        assert_eq!(FDEMON_CONFIG_NAME, "Flutter (fdemon)");
    }
}
