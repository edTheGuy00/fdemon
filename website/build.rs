use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
struct VersionEntry {
    version: Option<String>,
    timestamp: Option<i64>,
    commits: Vec<Commit>,
}

#[derive(Deserialize)]
struct Commit {
    message: String,
    group: Option<String>,
    scope: Option<String>,
}

/// Group display order for the website changelog (matches cliff.toml commit_parsers sequence).
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

fn epoch_to_date(epoch: i64) -> String {
    const SECS_PER_DAY: i64 = 86_400;
    let days = epoch / SECS_PER_DAY;

    // Civil date from day count (algorithm from Howard Hinnant)
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}")
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let json_path = Path::new(&manifest_dir).join("changelog.json");
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("changelog_generated.rs");

    println!("cargo::rerun-if-changed=changelog.json");

    let code = if json_path.exists() {
        let json = fs::read_to_string(&json_path).expect("failed to read changelog.json");
        let entries: Vec<VersionEntry> =
            serde_json::from_str(&json).expect("failed to parse changelog.json");
        generate_entries(&entries)
    } else {
        // Fallback: empty changelog for local dev without the JSON file
        "vec![]".to_string()
    };

    fs::write(&out_path, code).expect("failed to write changelog_generated.rs");
}

fn generate_entries(entries: &[VersionEntry]) -> String {
    let mut out = String::from("vec![\n");

    for entry in entries {
        let version = match &entry.version {
            Some(v) => v.strip_prefix('v').unwrap_or(v),
            None => continue, // skip unreleased
        };

        let date = entry
            .timestamp
            .map(epoch_to_date)
            .unwrap_or_else(|| "unknown".to_string());

        // Group commits by their `group` field
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

        // Sort groups by canonical order, with alphabetical tiebreak for reproducibility
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
