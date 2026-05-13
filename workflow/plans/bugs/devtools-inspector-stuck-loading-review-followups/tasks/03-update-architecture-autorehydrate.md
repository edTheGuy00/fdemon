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

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Removed `AutoRehydrate` bullet; updated `FetchTrigger` intro sentence to list only `Initial` and `Refresh` |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: YES/NO/N/A

### Notable Decisions/Tradeoffs

1. **Targeted removal only**: Removed the `AutoRehydrate` bullet and updated the introductory sentence enumerating variants. The surrounding paragraph (`has_ever_rendered_tree` cross-reference) needed no changes and was left intact.
