## Task: Update ARCHITECTURE.md After AutoRehydrate Removal

**Agent:** doc_maintainer

**Objective**: Remove the `AutoRehydrate` bullet from `docs/ARCHITECTURE.md` and confirm the `FetchTrigger` description matches the new two-variant code.

**Depends on**: 02-remove-autorehydrate-variant

**Estimated Time**: 0.25 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` — remove the `AutoRehydrate` bullet (around line 933) and update the surrounding paragraph to describe only `Initial` and `Refresh`

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules
- `crates/fdemon-app/src/handler/mod.rs` — verify the post-removal enum shape

### Change Context

After task 02 removes `FetchTrigger::AutoRehydrate`, the ARCHITECTURE.md description must reflect the actual codebase. The current text (line 933) reads:

```
- `AutoRehydrate` — background refresh triggered when the Inspector panel becomes
  visible again after a panel switch; follows the same bypass logic as `Refresh`.
```

This line is removed entirely. The surrounding paragraph should describe `FetchTrigger` as a two-variant enum (`Initial` polls; `Refresh` skips the poll).

### Acceptance Criteria

1. The `AutoRehydrate` bullet is removed from `docs/ARCHITECTURE.md`.
2. The paragraph describing `FetchTrigger` accurately reflects only `Initial` and `Refresh`.
3. No content-boundary violations (architecture content stays in ARCHITECTURE.md; no code samples added).
4. Cross-references in the section remain valid.

### Notes

- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.
- Make a targeted edit; do not rewrite the entire section.
- This is purely a documentation sync — the code change in task 02 is the source of truth.
