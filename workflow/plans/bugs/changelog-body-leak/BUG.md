# Bugfix Plan: Website Changelog Shows Full Commit Bodies

## TL;DR

Squash-merged PRs produce commits whose `message` field contains the full PR body (all individual commit messages). The website's `build.rs` uses `commit.message` verbatim without stripping to the first line, while `cliff.toml`'s Markdown template correctly applies `| split(pat="\n") | first`. The fix is to mirror that filter in `build.rs` and optionally configure `cliff.toml` to strip bodies from the JSON context too.

## Bug Reports

### Bug 1: Changelog entries show entire squash-merge bodies as run-on paragraphs

**Symptom:** Post-v0.1.0 changelog entries on the website display massive paragraphs containing every individual commit message from squash-merged PRs, instead of clean one-line descriptions.

**Expected:** Each changelog entry should show only the PR title (first line of the commit message), matching the clean v0.1.0 style.

**Root Cause Analysis:**

1. GitHub squash-merges produce a commit with:
   - **Subject line**: `PR Title (#N)`
   - **Body**: all individual commit messages separated by `\n`

2. `git cliff --context` outputs raw JSON where `commit.message` contains the full raw git message (subject + body) for non-conventional commits that land in "Other Changes".

3. `cliff.toml` line 18 applies `{{ commit.message | split(pat="\n") | first | upper_first }}` for the Markdown template — this correctly strips the body. But `git cliff --context` (used by the website) exports raw commit data **without** applying the template.

4. `website/build.rs:138` uses the message verbatim:
   ```rust
   let desc = escape(&upper_first(&commit.message));
   ```
   No `lines().next()` or equivalent first-line extraction.

5. The `escape()` function (line 64-66) escapes backslashes and quotes but **not** newlines. Multi-line messages become run-on text when rendered in HTML `<span>` tags (newlines collapse to spaces).

**Affected Files:**
- `website/build.rs:138` — missing first-line extraction
- `cliff.toml` — `split_commits = false` means multi-line messages stay as single entries (correct for CHANGELOG.md due to template filter, but not for JSON context)

---

## Affected Modules

- `website/build.rs`: Add first-line extraction to match cliff.toml's `split(pat="\n") | first` behavior
- `cliff.toml` (optional): Could add `body` processing config, but the real fix is in build.rs

---

## Phases

### Phase 1: Strip commit bodies in build.rs — Critical

The minimal fix: extract only the first line of `commit.message` before using it as the changelog description.

**Steps:**

1. **Add first-line extraction in `generate_entries()`**
   - At `website/build.rs:138`, change:
     ```rust
     let desc = escape(&upper_first(&commit.message));
     ```
     to:
     ```rust
     let first_line = commit.message.lines().next().unwrap_or("").trim();
     let desc = escape(&upper_first(first_line));
     ```
   - This mirrors exactly what the cliff.toml Markdown template does: `commit.message | split(pat="\n") | first`

2. **Strip PR number suffix (optional enhancement)**
   - Squash-merge subjects look like `Feat/session resilience (#3)`. The `(#3)` PR number is noise on the website.
   - Could add a regex or simple trim to remove trailing ` (#N)` patterns.
   - Decision: defer to Phase 2 if desired.

3. **Add unit tests for `generate_entries()`**
   - Test with multi-line `message` containing `\n` — verify only first line is used
   - Test with single-line message — verify no change in behavior
   - Test with empty message — verify graceful handling

**Measurable Outcomes:**
- `cargo test -p fdemon-website` passes (or `cargo check` for the website crate)
- Generated `changelog_generated.rs` contains only single-line descriptions
- Website changelog displays clean one-line entries for all versions

---

### Phase 2 (Optional): PR number cleanup and scope extraction

For squash-merged PRs landing in "Other Changes", the commit subject is the PR branch name (e.g., `Feat/session resilience (#3)`). This could be improved:

1. Strip trailing ` (#N)` PR number references
2. Convert branch-name-style subjects to readable text (e.g., `Feat/session resilience` → `Session resilience`)
3. Consider extracting scope from branch prefix (e.g., `Feat/` → scope `feat`)

**Decision point:** This phase is cosmetic and can be deferred. The critical fix is Phase 1.

---

## Edge Cases & Risks

### Multi-line conventional commits
- **Risk:** Some conventional commits (e.g., `feat: add X\n\nBREAKING CHANGE: Y`) have meaningful bodies
- **Mitigation:** The first-line-only approach is correct here — the subject line contains the feature description, the body contains metadata that shouldn't be in the changelog. This matches cliff.toml's existing behavior.

### Empty messages
- **Risk:** A commit with an empty or whitespace-only message
- **Mitigation:** `lines().next().unwrap_or("")` handles this gracefully, producing an empty string which `upper_first` also handles.

### Existing v0.1.0 entries
- **Risk:** The fix could change v0.1.0 entries if they had multi-line messages
- **Mitigation:** v0.1.0 was tagged before squash-merge workflow was adopted. Individual commits have single-line subjects, so the fix is a no-op for v0.1.0.

---

## Task Dependency Graph

```
Phase 1
└── 01-strip-commit-bodies   (single task, the fix is surgical)
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `website/build.rs` extracts only the first line of `commit.message`
- [ ] Unit tests verify multi-line messages are truncated to first line
- [ ] `cargo check` for the website crate passes
- [ ] Manual verification: generate changelog.json locally, run build.rs, inspect output

---

## Milestone Deliverable

Website changelog displays clean, one-line entries for all releases — matching the quality of the manually-curated v0.1.0 changelog.
