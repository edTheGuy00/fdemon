## Task: Update `docs/KEYBINDINGS.md` for Performance / Memory Split

**Objective**: Document the new Memory tab's keymap in the unmanaged `docs/KEYBINDINGS.md` reference. Add a new "Memory Panel" subsection under the DevTools section, update the existing "Performance Panel" subsection to reflect the slimmed responsibilities, and document the new `m` letter shortcut and `Esc` deselection precedence.

**Depends on**: 03-extract-memory-handlers-and-widgets

**Agent:** implementor (KEYBINDINGS.md is unmanaged — see plan §11)

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**
- `docs/KEYBINDINGS.md`

**Files Read (Dependencies):**
- `workflow/plans/features/devtools-performance-memory-split/phase-1/tasks/03-extract-memory-handlers-and-widgets.md` — the canonical keymap.
- `docs/KEYBINDINGS.md` — current DevTools section structure (find around the existing Performance Panel heading).

### Details

#### 1. DevTools sub-tab shortcuts table

Locate the DevTools shortcuts table (the section that lists `i`, `p`, `n`). Add `m`:

```markdown
| Key | Action |
|-----|--------|
| `i` | Switch to Inspector panel |
| `p` | Switch to Performance panel |
| `m` | Switch to Memory panel (NEW) |
| `n` | Switch to Network panel |
```

#### 2. Performance Panel section — trim memory references

Update the Performance Panel subsection. Remove references to memory chart, allocation table, and the `s` (sort) binding (which is now on Memory). The Performance Panel keymap becomes:

```markdown
### Performance Panel

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Cycle focus between Frame Chart and Details Tab |
| `←` / `→` | Select previous / next frame |
| `↑` / `k` | Scroll focused section up |
| `↓` / `j` | Scroll focused section down |
| `PageUp` / `PageDown` | Page-scroll focused section |
| `Home` / `End` | Jump to oldest / live edge |
| `Esc` | Deselect frame; or, if no frame selected, return to Logs |
| `Ctrl+p` | Toggle performance overlay on device |
| `b` | Open DevTools in browser |
```

#### 3. NEW Memory Panel section

Add a new subsection directly after Performance Panel:

```markdown
### Memory Panel

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Cycle focus between Memory Chart and Allocation List |
| `↑` / `k` | Scroll focused section up |
| `↓` / `j` | Scroll focused section down |
| `PageUp` / `PageDown` | Page-scroll focused section |
| `Home` / `End` | Jump to oldest / live edge of chart, or first / last alloc row |
| `s` | Toggle allocation sort (By Size ↔ By Instances) |
| `Esc` | Deselect alloc row; or, if no row selected, return to Logs |
| `b` | Open DevTools in browser |
```

#### 4. Migration note

Add a brief note at the top of the DevTools section (above the sub-tab table) — one sentence:

```markdown
> **Migration note (2026-05):** the previous Performance panel's memory chart and
> allocation table moved to a new Memory panel (`m`). The `s` (sort) binding
> moved with them.
```

(Use the current date — 2026-05-18 → "2026-05" is the migration month.)

### Acceptance Criteria

1. `docs/KEYBINDINGS.md` has a Memory Panel section with the 8-row table above.
2. The Performance Panel section no longer references memory chart, allocation table, or the `s` key.
3. The DevTools sub-tab table lists all four panels (`i`, `p`, `m`, `n`).
4. The migration note is present at the top of the DevTools section.
5. Markdown renders correctly (run `cat docs/KEYBINDINGS.md | head -100` to spot-check structure).

### Testing

This is a documentation-only task — no automated tests. Manual verification:

```bash
# Spot-check the rendered structure
grep -n "Memory Panel\|Performance Panel\|^####\|^### " docs/KEYBINDINGS.md
```

### Notes

- KEYBINDINGS.md is unmanaged (see plan §11) — implementor can edit it directly without invoking the doc_maintainer agent.
- Keep the existing markdown style: H3 sections, tables with `Key | Action`.
- Do NOT modify ARCHITECTURE.md, CODE_STANDARDS.md, or DEVELOPMENT.md — those have stricter content boundaries; ARCHITECTURE.md updates are owned by T05.

---

## Completion Summary

**Status:** Not Started
**Branch:** TBD

### Files Modified

| File | Changes |
|------|---------|
| | |

### Notable Decisions/Tradeoffs

1. **<Decision>**: <Rationale and implications>

### Testing Performed

- Markdown spot-check — TBD

### Risks/Limitations

1. **<Risk>**: <Description and mitigation if any>
