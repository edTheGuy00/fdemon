## Task: PLAN.md cross-reference markdown hyperlink

**Objective:** Convert the prose backtick path reference to BUG.md in `workflow/plans/features/mouse-support/PLAN.md` into a proper relative markdown hyperlink. Trivial fix flagged by the per-task validator and the code-quality reviewer.

**Depends on:** None

**Agent:** implementor

**Estimated time:** 5 minutes

### Scope

**Files Modified (Write):**
- `workflow/plans/features/mouse-support/PLAN.md`: line 536 (search for "log-text-selection-broken/BUG.md").

**Files Read (Dependencies):** None

### Details

Current text (illustrative — verify by reading the file):

```markdown
The fix landed via the bugfix (`workflow/plans/bugs/log-text-selection-broken/BUG.md`) dropped …
```

The path is a backtick-quoted prose path, not a clickable hyperlink. Convert to a relative markdown link:

```markdown
The fix landed via the bugfix ([BUG.md](../../bugs/log-text-selection-broken/BUG.md)) dropped …
```

Verify the relative path is correct: from `workflow/plans/features/mouse-support/PLAN.md`, the relative path to `workflow/plans/bugs/log-text-selection-broken/BUG.md` is `../../bugs/log-text-selection-broken/BUG.md`.

### Acceptance Criteria

1. The reference to BUG.md in `workflow/plans/features/mouse-support/PLAN.md` is a clickable markdown hyperlink: `[BUG.md](../../bugs/log-text-selection-broken/BUG.md)`.
2. Clicking the link in any standard markdown renderer (GitHub, VS Code preview) navigates to BUG.md.
3. No other content changes.

### Testing

Open the file in a markdown renderer and click the link to verify navigation works.

### Notes

- This file is NOT under the doc_maintainer's allow-list (it's a workflow plan doc, not a core docs/* file). Implementor handles it directly.
