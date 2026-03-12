## Task: Strip trailing PR number suffix from changelog descriptions

**Objective**: Remove trailing ` (#N)` GitHub PR references from changelog entry descriptions. These are noise on the website — users don't need to see internal PR numbers.

**Depends on**: 01-strip-message-first-line

**Estimated Time**: 0.5 hours

### Scope

- `website/build.rs`: Add a `strip_pr_suffix()` helper, call it in the description pipeline

### Details

Squash-merged commits have subjects like:
- `Feat/session resilience (#3)`
- `Fix: config.toml watcher paths and auto_start settings (#21)`
- `Feature: native platform logs (#20)`

The trailing ` (#N)` is GitHub's auto-appended PR number. Strip it with a simple regex or string operation.

```rust
/// Strip trailing ` (#N)` PR reference from a commit subject.
fn strip_pr_suffix(s: &str) -> &str {
    // Match ` (#<digits>)` at end of string
    if let Some(idx) = s.rfind(" (#") {
        if s[idx..].ends_with(')') && s[idx + 3..s.len() - 1].chars().all(|c| c.is_ascii_digit()) {
            return &s[..idx];
        }
    }
    s
}
```

Apply in the description pipeline (after first-line extraction from task 01, before `upper_first`):

```rust
let first_line = commit.message.lines().next().unwrap_or("").trim();
let cleaned = strip_pr_suffix(first_line);
let desc = escape(&upper_first(cleaned));
```

### Acceptance Criteria

1. `Feat/session resilience (#3)` → `Feat/session resilience`
2. `fix: resolve crash (#42)` → `fix: resolve crash`
3. `Feature: native platform logs (#20)` → `Feature: native platform logs`
4. Messages without PR suffix are unchanged
5. Parenthesized text that isn't a PR number is preserved (e.g., `fix: handle (edge case)`)

### Testing

```rust
#[test]
fn strip_pr_suffix_removes_number() {
    assert_eq!(strip_pr_suffix("feat: add widget (#12)"), "feat: add widget");
}

#[test]
fn strip_pr_suffix_preserves_non_pr_parens() {
    assert_eq!(strip_pr_suffix("fix: handle (edge case)"), "fix: handle (edge case)");
}

#[test]
fn strip_pr_suffix_no_suffix() {
    assert_eq!(strip_pr_suffix("feat: add widget"), "feat: add widget");
}

#[test]
fn strip_pr_suffix_high_number() {
    assert_eq!(strip_pr_suffix("Feature: big change (#1234)"), "Feature: big change");
}
```

### Notes

- Use simple string operations rather than pulling in the `regex` crate — the pattern is trivial
- This runs after task 01's first-line extraction, so the input is always a single line

---

## Completion Summary

**Status:** Not Started
