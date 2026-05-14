## Task: Update Architecture Doc for Performance Interactivity

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` to describe `PerfSection`, the scroll-offset model, and the render-hint cells used by the Performance panel.

**Depends on**: Phase 3

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` "DevTools Subsystem" (around line 839 — Performance panel area):
  - Document `PerfSection` enum (FrameChart, MemoryChart, MemoryList) and how `focused_section` shapes input routing.
  - Mention scroll-offset model: Model A — "frames back from live edge"; resets to 0 on `End`.
  - Note the three render-hint `Cell<usize>` fields used for visible-width/height feedback (with cross-reference to CODE_STANDARDS.md Principle 3).
  - Note the frame-history capacity bump (300 → 1800).

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: Content boundary rules.
- Phase 3 implementation files for accuracy.

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` accurately describes the new focus + scroll + render-hint model.
2. No content boundary violations (no code-style content, no build commands).
3. Cross-references valid.
4. Targeted edits — do not rewrite the whole DevTools section.

### Notes

- This is a managed core doc — only `doc_maintainer` agent may edit.
- Follow content boundaries strictly per `~/.claude/skills/doc-standards/schemas.md`.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Added "Performance Panel Interactivity" subsection to DevTools Subsystem section: documents `PerfSection` enum, live-edge scroll-offset model, three render-hint `Cell<usize>` fields (with cross-reference to CODE_STANDARDS.md Principle 3), and frame-history capacity bump to 1800. |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: YES/NO/N/A: N/A

### Notable Decisions/Tradeoffs

1. **Subsection placement**: Inserted the new subsection at the end of the DevTools Subsystem section (after "Browser DevTools URL"), before the section separator, to keep all Performance panel content together without disrupting existing subsections.
2. **Table for render-hint fields**: Used a compact table to list the three `Cell<usize>` fields and their purposes rather than inline prose, matching the style of other field reference tables in the document.
