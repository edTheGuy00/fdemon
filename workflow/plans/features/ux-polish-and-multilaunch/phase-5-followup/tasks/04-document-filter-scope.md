## Task: Document that `is_supported` filtering is dialog-scoped by design (M2)

**Objective**: Resolve the MAJOR consistency finding (M2) by *documenting* the intended
scope boundary rather than changing code. The Phase 5 `is_supported` filter applies only in
the new-session dialog's Connected tab; `find_auto_launch_target` (auto-start, cached
selection, first-config, bare run) and the headless path are exempt **by design**. Record
this in `docs/REVIEW_FOCUS.md` so the divergence is not later mistaken for a bug and "fixed."

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `docs/REVIEW_FOCUS.md` (unmanaged doc — implementor may edit directly; not a `doc_maintainer` core doc)

**Files Read (Dependencies):**
- `crates/fdemon-app/src/spawn.rs`: `find_auto_launch_target` and its tiers (read-only, to describe accurately).

### Details

Add a short subsection under the "Approved Optimizations" / project-specific concerns area of
`docs/REVIEW_FOCUS.md` (alongside the other "Approved Exception" notes). It must state:

- The `Device::is_supported` filter is applied **only** at the new-session dialog chokepoint
  `group_connected_devices` (`fdemon-app/src/new_session_dialog/device_groups.rs`). This
  governs the dialog's Connected tab, its flat list, cursor, multi-select, and click-regions.
- `find_auto_launch_target` (`fdemon-app/src/spawn.rs`) and the headless launch path
  **intentionally do not** consult `is_supported`. These non-interactive paths resolve a
  device from the full discovered list (`devices.first()` / `find_device` / cached
  `last_device`). This matches the Phase 5 scope ("Connected tab only") and the existing
  contract in `find_auto_launch_target`'s own doc ("callers should pre-filter").
- **Reviewer guidance:** Do **not** flag the auto-launch/headless paths' lack of an
  `is_supported` check as a bug. If a future product decision makes "never launch an
  unrunnable target" a system-wide invariant, that is a **new feature task** (introduce a
  shared `Device::is_runnable()` helper, apply a *default-true fallback* filter in
  `find_auto_launch_target` — never filter to empty — and add tests for an unsupported
  `devices.first()` and an unsupported cached `last_device`), not a regression fix.

Match the tone/format of the existing "Approved Exception" entries in the file (heading +
short rationale + explicit reviewer instruction). Keep it concise (one short subsection).

### Acceptance Criteria

1. `docs/REVIEW_FOCUS.md` contains a new subsection documenting that `is_supported` filtering is dialog-scoped and the auto-launch/headless paths are exempt by design.
2. The note names the chokepoint (`group_connected_devices`) and the exempt path (`find_auto_launch_target` / headless) accurately.
3. The note includes explicit reviewer guidance (do not flag as a bug; system-wide would be a new feature task with the listed shape).
4. No source-code changes; no other docs touched.

### Testing

- Documentation-only; no build/test impact. Verify the file renders as valid Markdown and the new heading fits the document's existing structure.

### Notes

- Decision recorded during Phase 5 review triage: **dialog-only scope** chosen over extending
  the filter system-wide. See `workflow/reviews/features/phase-5-runnable-filtering/ACTION_ITEMS.md` item M2.
- `docs/REVIEW_FOCUS.md` is explicitly an unmanaged doc per the planner's doc-routing rules, so
  this stays an `implementor` task (no `doc_maintainer` involvement, no core-doc boundary check).
