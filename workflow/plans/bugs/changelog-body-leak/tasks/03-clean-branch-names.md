## Task: Clean branch-name-style subjects into readable descriptions

**Objective**: Convert branch-name-style commit subjects (from squash-merged PRs) into clean, human-readable changelog descriptions. These land in the "Other Changes" group because they don't match conventional commit parsers.

**Depends on**: 01-strip-message-first-line

**Estimated Time**: 0.5-1 hour

### Scope

- `website/build.rs`: Add a `clean_subject()` helper, call it in the description pipeline

### Details

Post-v0.1.0 squash-merge subjects follow these patterns:

| Raw subject | Desired output |
|---|---|
| `Feat/session resilience` | `Session resilience` |
| `Feat/responsive session dialog` | `Responsive session dialog` |
| `Feat/auto changelog website` | `Auto changelog website` |
| `Fix/release branch protection` | `Release branch protection` |
| `Feature: native platform logs` | `Native platform logs` |
| `Feature: Native DAP server for IDE debugging` | `Native DAP server for IDE debugging` |
| `Fix: config.toml watcher paths and auto_start settings` | `Config.toml watcher paths and auto_start settings` |
| `Fix: extra args not passed` | `Extra args not passed` |
| `Fix: release workflow` | `Release workflow` |

The patterns to handle:

1. **`Type/description`** (branch-name style): Strip the prefix up to and including the first `/`, trim
2. **`Type: description`** (title-case conventional-ish): These already get parsed as conventional commits by git-cliff when lowercase (`fix:`, `feat:`), but title-case variants (`Fix:`, `Feature:`) fall through. Strip the prefix up to and including `: `, trim.

```rust
/// Clean a commit subject by stripping common prefixes.
///
/// Handles branch-name style (`Feat/description`) and title-case
/// conventional-ish style (`Fix: description`).
fn clean_subject(s: &str) -> &str {
    // Known prefixes that indicate the real description follows
    let prefixes = [
        "feat/", "fix/", "feature/", "chore/", "refactor/", "docs/", "test/",
        "Feat/", "Fix/", "Feature/", "Chore/", "Refactor/", "Docs/", "Test/",
        "Feature: ", "Fix: ",
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
```

Apply in the description pipeline (after strip_pr_suffix from task 02, before `upper_first`):

```rust
let first_line = commit.message.lines().next().unwrap_or("").trim();
let no_pr = strip_pr_suffix(first_line);
let cleaned = clean_subject(no_pr);
let desc = escape(&upper_first(cleaned));
```

**Note:** Only apply `clean_subject` to commits in the "Other Changes" group, since conventional commits already have their prefix stripped by git-cliff's conventional commit parser. Applying it to all commits would double-strip `feat:` entries.

### Acceptance Criteria

1. `Feat/session resilience` → `Session resilience`
2. `Fix/release branch protection` → `Release branch protection`
3. `Feature: native platform logs` → `Native platform logs`
4. `Fix: extra args not passed` → `Extra args not passed`
5. Already-clean subjects (e.g., `resolve crash on startup`) are unchanged
6. Conventional commit subjects (already stripped by git-cliff) are not affected
7. `upper_first` is applied after cleaning, ensuring consistent capitalization

### Testing

```rust
#[test]
fn clean_branch_name_feat() {
    assert_eq!(clean_subject("Feat/session resilience"), "session resilience");
}

#[test]
fn clean_branch_name_fix() {
    assert_eq!(clean_subject("Fix/release branch protection"), "release branch protection");
}

#[test]
fn clean_title_case_feature() {
    assert_eq!(clean_subject("Feature: native platform logs"), "native platform logs");
}

#[test]
fn clean_title_case_fix() {
    assert_eq!(clean_subject("Fix: extra args not passed"), "extra args not passed");
}

#[test]
fn clean_already_clean() {
    assert_eq!(clean_subject("resolve crash on startup"), "resolve crash on startup");
}

#[test]
fn clean_lowercase_conventional_not_stripped() {
    // These are already handled by git-cliff's conventional parser,
    // but verify the function doesn't break them
    assert_eq!(clean_subject("add widget tree support"), "add widget tree support");
}

#[test]
fn integration_full_pipeline() {
    // Full pipeline: first_line → strip_pr → clean_subject → upper_first
    let raw = "Feat/session resilience (#3)\n\nLong body here";
    let first_line = raw.lines().next().unwrap().trim();
    let no_pr = strip_pr_suffix(first_line);
    let cleaned = clean_subject(no_pr);
    let result = upper_first(cleaned);
    assert_eq!(result, "Session resilience");
}
```

### Notes

- `clean_subject` should only apply to "Other Changes" entries to avoid double-stripping conventional commits that git-cliff already parsed
- The `upper_first` call after `clean_subject` ensures consistent capitalization regardless of the original casing
- Case-insensitive prefix matching could be used instead of listing both cases, but explicit prefixes are clearer and avoid pulling in extra dependencies
- Hyphenated branch names (e.g., `feat/add-new-widget`) are not currently seen in this repo's history but the function handles them naturally since it just strips the prefix

---

## Completion Summary

**Status:** Not Started
