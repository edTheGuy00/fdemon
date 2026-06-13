//! Format-preserving, minimal TOML writes for struct-backed config files.
//!
//! The settings panel persists whole structs (`Settings`, `UserPreferences`),
//! but naively re-serialising them with `toml::to_string_pretty` destroys the
//! user's file: comments vanish, key order is rewritten, and any section the
//! struct doesn't model is dropped. That overwrites hand-crafted config the
//! user never touched through the UI.
//!
//! [`merge_into_existing`] avoids that. Given the existing file text, a
//! `baseline` value (what the editor loaded — i.e. the on-disk state), and the
//! `current` value (the desired state), it edits the file *in place* via
//! `toml_edit`, writing ONLY the leaf keys whose value differs between baseline
//! and current. Everything else — comments, blank lines, key ordering, and keys
//! the struct does not model — is preserved byte-for-byte.

use toml_edit::{DocumentMut, Item, Table};

/// Produce the new file contents for a struct-backed TOML config.
///
/// `existing` is the current file text (may be empty for a new file).
/// `baseline` is the value the editor started from (the on-disk state, parsed
/// with the same path the loader uses). `current` is the desired value.
///
/// Only leaves that differ between `baseline` and `current` are written; all
/// other file content is preserved. Returns the rendered document.
///
/// # Errors
///
/// Returns `Err` if `existing` is non-empty but is not valid TOML. The caller
/// should surface this rather than overwrite — refusing to clobber a file the
/// user is mid-editing is safer than replacing it wholesale.
pub fn merge_into_existing(
    existing: &str,
    baseline: &toml::Value,
    current: &toml::Value,
) -> Result<String, toml_edit::TomlError> {
    let mut doc = if existing.trim().is_empty() {
        DocumentMut::new()
    } else {
        existing.parse::<DocumentMut>()?
    };

    let base_table = baseline.as_table();
    if let Some(cur_table) = current.as_table() {
        merge_table(doc.as_table_mut(), base_table, cur_table);
    }

    Ok(doc.to_string())
}

/// Recursively write changed leaves from `current` into `doc`, using `baseline`
/// to decide what changed. Sub-tables are recursed into so a single changed key
/// deep in a section does not rewrite its siblings.
fn merge_table(doc: &mut Table, baseline: Option<&toml::Table>, current: &toml::Table) {
    // Update or add the keys the struct models, writing only what changed.
    for (key, cur_val) in current {
        let base_val = baseline.and_then(|b| b.get(key));

        if let toml::Value::Table(cur_sub) = cur_val {
            // Ensure a sub-table exists at this key, then recurse.
            if !doc.get(key).map(Item::is_table).unwrap_or(false) {
                doc.insert(key, Item::Table(Table::new()));
            }
            let base_sub = base_val.and_then(toml::Value::as_table);
            if let Some(Item::Table(doc_sub)) = doc.get_mut(key) {
                merge_table(doc_sub, base_sub, cur_sub);
            }
        } else if base_val != Some(cur_val) {
            // Leaf (scalar or array) that changed. Mutate the value in place
            // when the key already exists so the key's position and the value's
            // decor (notably a trailing inline comment) are preserved; only a
            // genuinely new key is appended.
            let new_value = to_edit_value(cur_val);
            match doc.get_mut(key) {
                Some(Item::Value(existing)) => {
                    let decor = existing.decor().clone();
                    *existing = new_value;
                    *existing.decor_mut() = decor;
                }
                _ => {
                    doc.insert(key, Item::Value(new_value));
                }
            }
        }
    }

    // Remove keys the struct models but `current` no longer contains — e.g. an
    // `Option` field cleared to `None`. Keys absent from `baseline` (hand-written
    // sections the struct does not model) are never in this set, so they survive.
    if let Some(base) = baseline {
        let stale: Vec<String> = base
            .keys()
            .filter(|k| !current.contains_key(*k))
            .cloned()
            .collect();
        for key in stale {
            doc.remove(&key);
        }
    }
}

/// Convert a `toml::Value` into a `toml_edit::Value` (default decor).
fn to_edit_value(v: &toml::Value) -> toml_edit::Value {
    use toml_edit::Value as Ev;
    match v {
        toml::Value::String(s) => Ev::from(s.clone()),
        toml::Value::Integer(i) => Ev::from(*i),
        toml::Value::Float(f) => Ev::from(*f),
        toml::Value::Boolean(b) => Ev::from(*b),
        // Settings structs use no TOML datetimes; render defensively as a string.
        toml::Value::Datetime(d) => Ev::from(d.to_string()),
        toml::Value::Array(a) => {
            let mut arr = toml_edit::Array::new();
            for e in a {
                arr.push(to_edit_value(e));
            }
            Ev::Array(arr)
        }
        toml::Value::Table(t) => {
            let mut inline = toml_edit::InlineTable::new();
            for (k, vv) in t {
                inline.insert(k, to_edit_value(vv));
            }
            Ev::InlineTable(inline)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn val(s: &str) -> toml::Value {
        s.parse::<toml::Value>().unwrap()
    }

    #[test]
    fn unchanged_value_leaves_file_byte_identical() {
        let existing = "# my config\n[ui]\ntheme = \"dark\"  # keep this\n";
        let baseline = val("[ui]\ntheme = \"dark\"\n");
        let current = baseline.clone();
        let out = merge_into_existing(existing, &baseline, &current).unwrap();
        assert_eq!(out, existing, "a no-op save must not alter the file");
    }

    #[test]
    fn changed_leaf_preserves_comments_and_siblings() {
        let existing = "\
# Top comment
[behavior]
confirm_quit = true  # ask first

[ui]
# theme picker
theme = \"dark\"
";
        let baseline = val("[behavior]\nconfirm_quit = true\n[ui]\ntheme = \"dark\"\n");
        let current = val("[behavior]\nconfirm_quit = false\n[ui]\ntheme = \"dark\"\n");
        let out = merge_into_existing(existing, &baseline, &current).unwrap();

        assert!(
            out.contains("confirm_quit = false"),
            "changed value written"
        );
        assert!(out.contains("# Top comment"), "header comment preserved");
        assert!(out.contains("# ask first"), "inline comment preserved");
        assert!(
            out.contains("# theme picker"),
            "untouched section comment preserved"
        );
        assert!(
            out.contains("theme = \"dark\""),
            "untouched sibling preserved"
        );
    }

    #[test]
    fn unmodelled_sections_are_preserved() {
        // The struct only knows [mcp]; a hand-written [secret] section must survive.
        let existing = "[secret]\ntoken = \"abc\"  # do not touch\n\n[mcp]\nport = 3939\n";
        let baseline = val("[mcp]\nport = 3939\n");
        let current = val("[mcp]\nport = 4040\n");
        let out = merge_into_existing(existing, &baseline, &current).unwrap();
        assert!(out.contains("[secret]"), "unmodelled section kept");
        assert!(out.contains("token = \"abc\""));
        assert!(out.contains("# do not touch"));
        assert!(out.contains("port = 4040"), "modelled change applied");
    }

    #[test]
    fn new_key_added_without_disturbing_existing() {
        // current adds behavior.confirm_quit; file has no [behavior] table yet.
        let existing = "[ui]\ntheme = \"dark\"\n";
        let baseline = val("[ui]\ntheme = \"dark\"\n");
        let current = val("[ui]\ntheme = \"dark\"\n[behavior]\nconfirm_quit = false\n");
        let out = merge_into_existing(existing, &baseline, &current).unwrap();
        assert!(out.contains("theme = \"dark\""));
        assert!(out.contains("confirm_quit = false"));
    }

    #[test]
    fn empty_existing_writes_only_changed_leaves() {
        let existing = "";
        let baseline = val("port = 3939\nenabled = true\n");
        let current = val("port = 4040\nenabled = true\n");
        let out = merge_into_existing(existing, &baseline, &current).unwrap();
        assert!(out.contains("port = 4040"));
        assert!(
            !out.contains("enabled"),
            "unchanged leaf is not materialised in a fresh file"
        );
    }

    #[test]
    fn nested_change_does_not_rewrite_sibling_subkeys() {
        let existing = "\
[devtools]
auto_open = false  # keep
browser = \"firefox\"  # keep too
";
        let baseline = val("[devtools]\nauto_open = false\nbrowser = \"firefox\"\n");
        let current = val("[devtools]\nauto_open = true\nbrowser = \"firefox\"\n");
        let out = merge_into_existing(existing, &baseline, &current).unwrap();
        assert!(out.contains("auto_open = true"));
        assert!(out.contains("browser = \"firefox\""));
        assert!(
            out.contains("# keep too"),
            "sibling inline comment preserved"
        );
    }

    #[test]
    fn changed_array_is_replaced_as_a_whole() {
        let existing = "[watcher]\npaths = [\"lib\"]\n";
        let baseline = val("[watcher]\npaths = [\"lib\"]\n");
        let current = val("[watcher]\npaths = [\"lib\", \"test\"]\n");
        let out = merge_into_existing(existing, &baseline, &current).unwrap();
        assert!(out.contains("\"test\""), "array change applied");
    }

    #[test]
    fn cleared_modeled_key_is_removed_but_unmodelled_kept() {
        // baseline has `last_device` (struct-modeled, now cleared) AND a
        // hand-written `[notes]` section the struct never models.
        let existing = "last_device = \"emulator-5554\"\n\n[notes]\nmine = \"keep\"\n";
        let baseline = val("last_device = \"emulator-5554\"\n");
        let current = val(""); // last_device cleared (Option -> None)
        let out = merge_into_existing(existing, &baseline, &current).unwrap();
        assert!(
            !out.contains("last_device"),
            "cleared modeled key removed: {out}"
        );
        assert!(out.contains("[notes]"), "unmodelled section preserved");
        assert!(out.contains("mine = \"keep\""));
    }

    #[test]
    fn malformed_existing_is_an_error_not_a_clobber() {
        let existing = "[mcp\nthis is not toml";
        let baseline = val("port = 1\n");
        let current = val("port = 2\n");
        assert!(merge_into_existing(existing, &baseline, &current).is_err());
    }
}
