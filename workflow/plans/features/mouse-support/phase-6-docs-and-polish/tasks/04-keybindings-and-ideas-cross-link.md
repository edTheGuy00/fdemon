## Task: KEYBINDINGS.md Cross-Link and IDEAS.md Strike

**Objective**: Add a top-of-file callout to `docs/KEYBINDINGS.md` pointing readers to `docs/MOUSE.md`; remove the "Mouse Support" entry from `docs/IDEAS.md` Deferred Features list.

**Depends on**: None

**Estimated Time**: 0.25h

### Scope

**Files Modified (Write):**
- `docs/KEYBINDINGS.md`: Insert a one-paragraph callout near the top of the file (immediately after the `#` heading or after the existing intro paragraph), pointing at `docs/MOUSE.md`.
- `docs/IDEAS.md`: Remove the entire "### 2. Mouse Support" entry (priority/complexity, description, potential features, implementation notes, why deferred — currently lines ~30–53). Renumber subsequent entries OR leave numbering as-is per the existing IDEAS.md convention (whichever the rest of the file uses; do not invent a new convention).

**Files Read (Dependencies):**
- `docs/MOUSE.md` — the callout's link target. Verify the path / heading anchor is correct.
- `docs/IDEAS.md` — current numbering convention; verify how to handle renumbering vs. preservation.

### Details

#### KEYBINDINGS.md callout

Insert near the top of the file:

```markdown
> **Mouse interactions** are documented separately in [MOUSE.md](MOUSE.md), which covers
> wheel-scroll routing, click-to-activate semantics for the header / tabs / dialogs / DevTools,
> and the `[ui] enable_mouse` opt-out.
```

Place after the file's `#` heading and any existing one-line intro, before the first `## ...` mode section. Do not modify the keyboard binding tables themselves.

#### IDEAS.md entry removal

The "### 2. Mouse Support" block (currently around lines 30–53) is removed in its entirety:

- Heading line `### 2. Mouse Support`
- Priority / Complexity lines
- Body paragraph
- "**Potential Features**:" bullet list
- "**Implementation Notes**:" block
- "**Why Deferred**:" block
- The trailing `---` separator (unless it is shared with the next entry's header — preserve markdown structure).

After removal, scan IDEAS.md for orphan section dividers (`---`) and delete any double-divider artifacts. Verify subsequent entries (`### 3. Remote Development`, etc.) render correctly under their existing numbering.

**Numbering decision rule:** Look at whether the existing IDEAS.md entries are tightly numbered (1, 2, 3, ...) or loosely (skip-ok). If they are tightly numbered, renumber 3+ down by one. If they appear to use stable numbering (gaps tolerated for historical reasons), leave the numbers alone and just delete entry 2.

### Acceptance Criteria

1. `docs/KEYBINDINGS.md` has a top-of-file callout pointing at `MOUSE.md`. The callout uses standard markdown blockquote syntax and is placed after the `#` heading and before the first `##` section.
2. `docs/IDEAS.md` no longer contains the "Mouse Support" entry. No stray "Why Deferred" prose remains.
3. IDEAS.md numbering is internally consistent post-edit (either re-numbered or preserved per existing convention).
4. No other content in either file is modified — keyboard tables, other deferred features, IDEAS.md prose all survive intact.
5. `grep "Mouse Support" docs/IDEAS.md` returns no matches.
6. `grep -i "mouse" docs/KEYBINDINGS.md` returns the new callout (and only the new callout).

### Testing

```bash
# Verify removal:
grep -ni "mouse" docs/IDEAS.md   # Should return nothing.

# Verify callout:
grep -A 3 "MOUSE.md" docs/KEYBINDINGS.md

# Verify the file still parses as valid markdown (project does not commit a markdown linter; spot-check).
```

### Notes

- Both edits are tiny. They are bundled into one task because they are both trivially mechanical and share zero overlap with any other Phase 6 task.
- Do not preserve the "Mouse Support" entry as a struck-through placeholder. The PLAN.md success criteria explicitly says IDEAS.md "no longer lists Mouse Support as deferred."
- Do not add a "Mouse Support" entry to a "Shipped Features" or "Completed" section unless one already exists in IDEAS.md. Inventing such a section is out of scope.
- The callout's link uses a relative path (`MOUSE.md`), not a full URL, so it works both on GitHub-rendered markdown and on local file viewers.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-ad74a8bdc6a940992

### Files Modified

| File | Changes |
|------|---------|
| `docs/KEYBINDINGS.md` | Added blockquote callout pointing to MOUSE.md after the intro paragraph and before the first `##` section |
| `docs/IDEAS.md` | Removed entire "### 2. Mouse Support" entry; renumbered "Remote Development" to 2, "Plugin System" to 3 |

### Notable Decisions/Tradeoffs

1. **Tight renumbering**: IDEAS.md uses tight sequential numbering (1, 2, 3...), so after removing entry 2 (Mouse Support), Remote Development was renumbered to 2 and Plugin System to 3. This keeps the document internally consistent.
2. **Callout placement**: The blockquote is placed between the intro paragraph and the `---` horizontal rule that precedes the Table of Contents, which satisfies "after the `#` heading and before the first `##` section" per the acceptance criteria.

### Testing Performed

- `grep -ni "mouse" docs/IDEAS.md` - Returns no output (mouse entry fully removed)
- `grep -A 3 "MOUSE.md" docs/KEYBINDINGS.md` - Shows the new blockquote callout
- `grep -i "mouse" docs/KEYBINDINGS.md` - Returns only the new callout lines
- `grep "^### [0-9]" docs/IDEAS.md` - Shows 1, 2, 3 tight sequence

### Risks/Limitations

1. **Documentation-only changes**: No Rust code was modified; cargo quality gate is not applicable to this task.
