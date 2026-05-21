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

**Status:** Not Started
**Branch:** feat/version-check-banner-followup

### Files Modified

| File | Changes |
|------|---------|
| | |

### Notable Decisions/Tradeoffs

1. **<Decision>**: <Rationale>

### Testing Performed

- `grep -n "Approved Exception" docs/REVIEW_FOCUS.md` — Pending

### Risks/Limitations

1. **<Risk>**: <Description>
