## Task: Update all branch references from master/develop to main

**Objective**: Update all hardcoded branch references across the codebase so they point to `main` (the new trunk branch) instead of `master` or `develop`.

**Depends on**: None

**Wave**: 1 (parallel)

### Scope

- `install.sh`: **Edit** (2 lines)
- `README.md`: **Edit** (1 line)
- `website/src/pages/docs/installation.rs`: **Edit** (3 lines)

### Details

#### 1. `install.sh` — Lines 5-6 (header comments)

The header comments reference `master`. The `print_usage()` examples (lines 45, 48, 51) already use `main` — only the top comments are inconsistent.

```bash
# Line 5 — change:
#   curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/master/install.sh | bash
# To:
#   curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash

# Line 6 — change:
#   curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/master/install.sh | bash -s -- --version 0.2.0
# To:
#   curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash -s -- --version 0.2.0
```

#### 2. `README.md` — Line 35

```markdown
# Change:
curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/master/install.sh | bash
# To:
curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash
```

Note: Line 14 (`href="...blob/main/LICENSE"`) already uses `main` — no change needed.

#### 3. `website/src/pages/docs/installation.rs` — Lines 22, 31, 40

All three `CodeBlock` components embed `master` in the install URL:

```rust
// Line 22 — change master → main:
<CodeBlock code="curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash" />

// Line 31 — change master → main:
<CodeBlock code="curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash -s -- --version 0.1.0" />

// Line 40 — change master → main:
<CodeBlock code="FDEMON_INSTALL_DIR=/usr/local/bin curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash" />
```

#### Files already correct (no changes needed)

| File | Line(s) | Note |
|------|---------|------|
| `README.md` | 14 | GitHub blob URL already uses `/main/` |
| `install.sh` | 45, 48, 51 | `print_usage()` already uses `/main/` |
| `CONTRIBUTING.md` | 93 | Already says "Create a feature branch from `main`" |
| `.github/workflows/e2e.yml` | 5, 7 | Already uses `branches: [main]` — becomes correct after rename |
| `.github/workflows/release.yml` | — | Being fully rewritten in Task 01 |
| `.github/workflows/publish-site.yml` | — | Being edited in Task 02 |

### Acceptance Criteria

1. No `master` references remain in `install.sh`, `README.md`, or `website/src/pages/docs/installation.rs`
2. All install URLs use `/main/` as the branch
3. No unrelated changes to these files

### Testing

- Grep for `master` in the modified files: should return zero matches
- `cargo check -p flutter-demon-site` (if the website crate is part of the workspace) — but it's excluded, so just verify syntax visually

### Notes

- This is a simple find-and-replace task across 3 files (6 total line edits)
- GitHub automatically sets up redirects when branches are renamed, so existing `master` URLs will continue to work temporarily — but we should still update for correctness

---

## Completion Summary

**Status:** Not Started
