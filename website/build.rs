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

/// Canonical ordering for commit groups (matches cliff.toml parser order).
fn group_order(group: &str) -> usize {
    match group {
        "Features" => 0,
        "Bug Fixes" => 1,
        "Performance" => 2,
        "Refactoring" => 3,
        "Documentation" => 4,
        "Styling" => 5,
        "Testing" => 6,
        "Security" => 7,
        "Reverted" => 8,
        _ => 99,
    }
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

        // Sort groups by canonical order
        let mut sorted_groups: Vec<_> = groups.into_iter().collect();
        sorted_groups.sort_by_key(|(g, _)| group_order(g));

        out.push_str("    ChangelogEntry {\n");
        out.push_str(&format!("        version: \"{}\",\n", escape(version)));
        out.push_str(&format!("        date: \"{}\",\n", escape(&date)));
        out.push_str("        groups: vec![\n");

        for (group, commits) in &sorted_groups {
            out.push_str("            ChangelogGroup {\n");
            out.push_str(&format!(
                "                group: \"{}\",\n",
                escape(group)
            ));
            out.push_str("                changes: vec![\n");

            for commit in commits {
                let desc = upper_first(&escape(&commit.message));
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
