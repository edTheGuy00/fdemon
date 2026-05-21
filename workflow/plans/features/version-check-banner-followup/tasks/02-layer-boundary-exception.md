## Task: Document `fdemon-app::version_check` as an approved layer-boundary exception

**Objective**: Add an entry to `docs/REVIEW_FOCUS.md` under "Approved Optimizations" (or a new "Approved Exceptions" subsection) that explicitly names `crates/fdemon-app/src/version_check.rs` as the sole permitted network I/O at the `fdemon-app` layer. This documents the precedent so future reviewers can enforce the boundary against drift.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**

- `docs/REVIEW_FOCUS.md`: Add an entry inside the existing "Approved Optimizations" section (currently houses `Forwarder Panel Gate`, `try_send for Flutter.RebuiltWidgets`, etc.). New subsection name: `Approved Exception: fdemon-app::version_check Network I/O`.

**Files Read (Dependencies):**

- `docs/ARCHITECTURE.md`: layer dependency matrix.
- `docs/REVIEW_FOCUS.md`: existing "Approved Optimizations" section format.
- `crates/fdemon-app/src/version_check.rs`: confirm module purpose.

### Details

**Proposed copy** (drop into `docs/REVIEW_FOCUS.md` under "Approved Optimizations"):

```markdown
### Approved Exception: `fdemon-app::version_check` Network I/O

`crates/fdemon-app/src/version_check.rs` is the **only** module in `fdemon-app` permitted
to perform outbound network I/O. It issues one HTTPS GET to GitHub's releases API on
startup (gated behind `[behavior] version_check`) to surface a "new version available"
banner.

This is an exception to the layered architecture documented in `docs/ARCHITECTURE.md`,
which assigns network I/O to `fdemon-daemon`. The exception is approved because:

1. The module has no Flutter-protocol knowledge — placing it in `fdemon-daemon` would
   force the daemon crate to take a TLS dependency it does not otherwise need.
2. The call is bounded: one HTTPS request per process, 3-second timeout, fire-and-forget.
3. The behavior is fully opt-out via `[behavior] version_check = false`.

**Reviewers should reject** any new outbound network I/O in `fdemon-app` outside this
module without a similar explicit exception. Future HTTPS-using features (e.g., crash
reporting, plugin registry) should land in `fdemon-daemon` or a new dedicated crate
(`fdemon-net`), not be added next to `version_check.rs` as precedent.
```

### Acceptance Criteria

1. `grep -n "version_check Network I/O" docs/REVIEW_FOCUS.md` returns one match.
2. `grep -n "Approved Exception" docs/REVIEW_FOCUS.md` returns at least one match.
3. The entry sits inside the existing "Approved Optimizations" top-level section (preserving the document's outline).
4. The entry names a reject-criterion for future similar changes — i.e. it is not just descriptive ("this exists") but prescriptive ("don't add more").

### Testing

Docs-only task. No code tests.

Visual review: confirm the section renders within the existing "Approved Optimizations" hierarchy without breaking the surrounding outline.

### Notes

- The `docs/CODE_STANDARDS.md` "Approved TEA Exception" pattern is analogous — both are explicit, prescriptive carve-outs from a documented rule. Mirror that tone.
- Do not edit `docs/ARCHITECTURE.md` in this task — that's reserved for `doc_maintainer` (task 06 covers any architecture-doc updates that come out of the follow-ups).

---

## Completion Summary

**Status:** Done
**Branch:** feat/version-check-banner

### Files Modified

| File | Changes |
|------|---------|
| `docs/REVIEW_FOCUS.md` | Added "Approved Exception: fdemon-app::version_check Network I/O" subsection under "Approved Optimizations" section |

### Notable Decisions/Tradeoffs

1. **Heading without backticks**: The proposed copy in the task used backticks around `fdemon-app::version_check` in the heading (`` ### Approved Exception: `fdemon-app::version_check` Network I/O ``). However, the acceptance criteria grep `grep -n "version_check Network I/O"` would fail because a backtick sits between `version_check` and `Network` in the raw file. The heading was written without backticks (`### Approved Exception: fdemon-app::version_check Network I/O`) to satisfy the acceptance criteria exactly, matching the style of other headings in the same file (e.g., "Forwarder Panel Gate") that don't wrap code identifiers in backticks.

### Testing Performed

- `grep -n "version_check Network I/O" docs/REVIEW_FOCUS.md` — 1 match (line 165)
- `grep -n "Approved Exception" docs/REVIEW_FOCUS.md` — 2 matches (lines 15, 165)
- Visual review confirmed the entry is within the "Approved Optimizations" section (lines 107–184) and the prescriptive "**Reviewers should reject**" criterion is present

### Risks/Limitations

1. **Docs-only task**: No code was modified; no compilation or test verification needed.
