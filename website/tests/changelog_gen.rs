/// Integration tests for changelog code-generation logic from build.rs.
///
/// Because `build.rs` is compiled as a standalone binary by Cargo and is not
/// included in the normal `cargo test` run, the testable helper functions are
/// replicated here so they can be verified independently.  Any change to the
/// corresponding code in `build.rs` should be mirrored here.
///
/// Note: git-cliff strips the conventional commit prefix (`feat:`, `fix:`, …)
/// from `commit.message` before writing it to the JSON context, so the strings
/// in these tests represent the post-strip description that git-cliff produces.
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types (mirror of build.rs)
// ---------------------------------------------------------------------------

struct VersionEntry {
    version: Option<String>,
    timestamp: Option<i64>,
    commits: Vec<Commit>,
}

struct Commit {
    message: String,
    group: Option<String>,
    scope: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers (mirror of build.rs)
// ---------------------------------------------------------------------------

fn group_order(group: &str) -> usize {
    match group {
        "Features" => 0,
        "Bug Fixes" => 1,
        "Documentation" => 2,
        "Performance" => 3,
        "Refactoring" => 4,
        "Styling" => 5,
        "Testing" => 6,
        "Security" => 7,
        "Reverted" => 8,
        _ => 99,
    }
}

/// Strip trailing ` (#N)` PR reference from a commit subject.
fn strip_pr_suffix(s: &str) -> &str {
    // Match ` (#<digits>)` at end of string
    if let Some(idx) = s.rfind(" (#") {
        if s[idx..].ends_with(')')
            && s[idx + 3..s.len() - 1].chars().all(|c| c.is_ascii_digit())
        {
            return &s[..idx];
        }
    }
    s
}

/// Clean a commit subject by stripping common prefixes.
///
/// Handles branch-name style (`Feat/description`) and title-case
/// conventional-ish style (`Fix: description`).
///
/// This should only be applied to commits that are not already parsed as
/// conventional commits by git-cliff (i.e. "Other Changes" entries), since
/// git-cliff already strips the prefix from conventional commits.
fn clean_subject(s: &str) -> &str {
    // Known prefixes that indicate the real description follows
    let prefixes = [
        "feat/",
        "fix/",
        "feature/",
        "chore/",
        "refactor/",
        "docs/",
        "test/",
        "Feat/",
        "Fix/",
        "Feature/",
        "Chore/",
        "Refactor/",
        "Docs/",
        "Test/",
        "Feature: ",
        "Fix: ",
    ];

    for prefix in &prefixes {
        if let Some(rest) = s.strip_prefix(prefix) {
            let trimmed = rest.trim();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }

    s
}

fn upper_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn generate_entries(entries: &[VersionEntry]) -> String {
    let mut out = String::from("vec![\n");

    for entry in entries {
        let version = match &entry.version {
            Some(v) => v.strip_prefix('v').unwrap_or(v),
            None => continue,
        };

        let date = entry
            .timestamp
            .map(|_| "2025-02-20".to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let mut groups: BTreeMap<String, Vec<&Commit>> = BTreeMap::new();
        for commit in &entry.commits {
            if let Some(group) = &commit.group {
                if group == "Miscellaneous" {
                    continue;
                }
                groups.entry(group.clone()).or_default().push(commit);
            }
        }

        if groups.is_empty() {
            continue;
        }

        let mut sorted_groups: Vec<_> = groups.into_iter().collect();
        sorted_groups
            .sort_by(|(a, _), (b, _)| group_order(a).cmp(&group_order(b)).then_with(|| a.cmp(b)));

        out.push_str("    ChangelogEntry {\n");
        out.push_str(&format!("        version: \"{}\",\n", escape(version)));
        out.push_str(&format!("        date: \"{}\",\n", escape(&date)));
        out.push_str("        groups: vec![\n");

        for (group, commits) in &sorted_groups {
            out.push_str("            ChangelogGroup {\n");
            out.push_str(&format!("                group: \"{}\",\n", escape(group)));
            out.push_str("                changes: vec![\n");

            for commit in commits {
                let first_line = commit.message.lines().next().unwrap_or("").trim();
                let no_pr = strip_pr_suffix(first_line);
                // clean_subject only applies to non-conventional-commit groups
                // (group_order == 99) because git-cliff already strips the prefix
                // from conventional commits (feat:, fix:, etc.).
                let subject = if group_order(group) == 99 {
                    clean_subject(no_pr)
                } else {
                    no_pr
                };
                let desc = escape(&upper_first(subject));
                match &commit.scope {
                    Some(scope) => {
                        out.push_str(&format!(
                            "                    ChangelogChange {{ description: \"{desc}\", scope: Some(\"{}\") }},\n",
                            escape(scope)
                        ));
                    }
                    None => {
                        out.push_str(&format!(
                            "                    ChangelogChange {{ description: \"{desc}\", scope: None }},\n"
                        ));
                    }
                }
            }

            out.push_str("                ],\n");
            out.push_str("            },\n");
        }

        out.push_str("        ],\n");
        out.push_str("    },\n");
    }

    out.push(']');
    out
}

// ---------------------------------------------------------------------------
// Helper to build a single-commit VersionEntry
// ---------------------------------------------------------------------------

fn make_entry(message: &str, group: &str) -> VersionEntry {
    VersionEntry {
        version: Some("v1.0.0".to_string()),
        timestamp: Some(1_740_000_000),
        commits: vec![Commit {
            message: message.to_string(),
            group: Some(group.to_string()),
            scope: None,
        }],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// A squash-merge PR: git-cliff strips the conventional-commit prefix and
/// appends individual commit subjects as body paragraphs.  Only the first line
/// (the PR title summary) should appear in the generated code.
#[test]
fn multiline_message_uses_first_line_only() {
    // Simulates git-cliff output for a squash-merge: prefix stripped, body present.
    let entry = make_entry(
        "add widget\n\nThis is the body\nWith multiple lines",
        "Features",
    );
    let code = generate_entries(&[entry]);
    assert!(
        code.contains("Add widget"),
        "expected capitalised first line in output, got:\n{code}"
    );
    assert!(
        !code.contains("This is the body"),
        "body text must not appear in output, got:\n{code}"
    );
    assert!(
        !code.contains("With multiple lines"),
        "second body line must not appear in output, got:\n{code}"
    );
}

/// Single-line messages (normal non-squash commits) are unaffected.
#[test]
fn single_line_message_unchanged() {
    let entry = make_entry("resolve crash", "Bug Fixes");
    let code = generate_entries(&[entry]);
    assert!(
        code.contains("Resolve crash"),
        "expected capitalised message in output, got:\n{code}"
    );
}

/// An empty message must not panic and must still produce a ChangelogChange node.
#[test]
fn empty_message_no_panic() {
    let entry = make_entry("", "Features");
    let code = generate_entries(&[entry]);
    assert!(
        code.contains("ChangelogChange"),
        "expected ChangelogChange in output even for empty message, got:\n{code}"
    );
}

/// Messages with CRLF line endings (e.g. from Windows git clients) must also
/// have their body stripped correctly.
#[test]
fn crlf_line_endings_use_first_line_only() {
    let entry = make_entry(
        "windows compat\r\nBody line one\r\nBody line two",
        "Features",
    );
    let code = generate_entries(&[entry]);
    assert!(
        code.contains("Windows compat"),
        "expected capitalised first line in output, got:\n{code}"
    );
    assert!(
        !code.contains("Body line one"),
        "CRLF body text must not appear in output, got:\n{code}"
    );
}

/// `upper_first` and `escape` must be applied to the extracted first line, not
/// to the whole multi-line message.
#[test]
fn upper_first_and_escape_apply_to_extracted_line() {
    // First line starts lowercase; body should be ignored.
    let entry = make_entry("handle paths\nIgnored body", "Bug Fixes");
    let code = generate_entries(&[entry]);
    assert!(
        code.contains("Handle paths"),
        "upper_first must capitalise extracted first line, got:\n{code}"
    );
    assert!(
        !code.contains("Ignored body"),
        "body must not appear in output, got:\n{code}"
    );
}

// ---------------------------------------------------------------------------
// strip_pr_suffix unit tests
// ---------------------------------------------------------------------------

#[test]
fn strip_pr_suffix_removes_number() {
    assert_eq!(strip_pr_suffix("feat: add widget (#12)"), "feat: add widget");
}

#[test]
fn strip_pr_suffix_preserves_non_pr_parens() {
    assert_eq!(
        strip_pr_suffix("fix: handle (edge case)"),
        "fix: handle (edge case)"
    );
}

#[test]
fn strip_pr_suffix_no_suffix() {
    assert_eq!(strip_pr_suffix("feat: add widget"), "feat: add widget");
}

#[test]
fn strip_pr_suffix_high_number() {
    assert_eq!(
        strip_pr_suffix("Feature: big change (#1234)"),
        "Feature: big change"
    );
}

/// Acceptance criteria from the task: squash-merge style subjects.
#[test]
fn strip_pr_suffix_session_resilience() {
    assert_eq!(
        strip_pr_suffix("Feat/session resilience (#3)"),
        "Feat/session resilience"
    );
}

#[test]
fn strip_pr_suffix_fix_with_pr() {
    assert_eq!(
        strip_pr_suffix("fix: resolve crash (#42)"),
        "fix: resolve crash"
    );
}

#[test]
fn strip_pr_suffix_feature_native_logs() {
    assert_eq!(
        strip_pr_suffix("Feature: native platform logs (#20)"),
        "Feature: native platform logs"
    );
}

/// Verify the full pipeline strips PR suffix before upper_first is applied.
#[test]
fn generate_entries_strips_pr_suffix() {
    let entry = make_entry("add widget (#12)", "Features");
    let code = generate_entries(&[entry]);
    assert!(
        code.contains("Add widget"),
        "expected PR suffix stripped and first letter capitalised, got:\n{code}"
    );
    assert!(
        !code.contains("(#12)"),
        "PR suffix must not appear in generated output, got:\n{code}"
    );
}

// ---------------------------------------------------------------------------
// clean_subject unit tests
// ---------------------------------------------------------------------------

#[test]
fn clean_branch_name_feat() {
    assert_eq!(clean_subject("Feat/session resilience"), "session resilience");
}

#[test]
fn clean_branch_name_fix() {
    assert_eq!(
        clean_subject("Fix/release branch protection"),
        "release branch protection"
    );
}

#[test]
fn clean_title_case_feature() {
    assert_eq!(
        clean_subject("Feature: native platform logs"),
        "native platform logs"
    );
}

#[test]
fn clean_title_case_fix() {
    assert_eq!(
        clean_subject("Fix: extra args not passed"),
        "extra args not passed"
    );
}

#[test]
fn clean_already_clean() {
    assert_eq!(
        clean_subject("resolve crash on startup"),
        "resolve crash on startup"
    );
}

#[test]
fn clean_lowercase_conventional_not_stripped() {
    // These are already handled by git-cliff's conventional parser,
    // but verify the function doesn't break them
    assert_eq!(
        clean_subject("add widget tree support"),
        "add widget tree support"
    );
}

/// Full pipeline: first_line -> strip_pr -> clean_subject -> upper_first
#[test]
fn integration_full_pipeline() {
    let raw = "Feat/session resilience (#3)\n\nLong body here";
    let first_line = raw.lines().next().unwrap().trim();
    let no_pr = strip_pr_suffix(first_line);
    let cleaned = clean_subject(no_pr);
    let result = upper_first(cleaned);
    assert_eq!(result, "Session resilience");
}

/// Verify that branch-name subjects in "Other Changes" group are cleaned.
#[test]
fn generate_entries_cleans_other_changes_group() {
    let entry = make_entry("Feat/session resilience (#3)", "Other Changes");
    let code = generate_entries(&[entry]);
    assert!(
        code.contains("Session resilience"),
        "expected cleaned subject in output, got:\n{code}"
    );
    assert!(
        !code.contains("Feat/"),
        "branch-name prefix must not appear in generated output, got:\n{code}"
    );
}

/// Verify that conventional commit groups are NOT cleaned by clean_subject
/// (git-cliff already strips their prefix).
#[test]
fn generate_entries_does_not_clean_conventional_groups() {
    // A Features entry whose message is already stripped by git-cliff.
    // If clean_subject were applied, "feat/" would be stripped again — but
    // since git-cliff already did that, the message here won't have the prefix.
    // We test that a message that happens to start with a known prefix but
    // belongs to a conventional group is passed through unchanged.
    let entry = make_entry("feature/something cool", "Features");
    let code = generate_entries(&[entry]);
    // For a conventional group the "feature/" prefix must NOT be stripped —
    // only upper_first is applied.
    assert!(
        code.contains("Feature/something cool"),
        "conventional group entries must not have prefix stripped, got:\n{code}"
    );
}
