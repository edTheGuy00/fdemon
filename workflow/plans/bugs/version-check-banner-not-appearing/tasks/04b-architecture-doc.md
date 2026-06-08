# Task 04b — Update ARCHITECTURE.md for the banner data-flow change

**Agent:** doc_maintainer
**Depends on:** 01, 02, 03
**Estimated:** 0.5h

## Objective

Reflect the corrected version-check data flow in `docs/ARCHITECTURE.md`: the banner is no longer
coupled to the New Session Dialog, and the cache is version-keyed.

## Files (Write)

- `docs/ARCHITECTURE.md`

## Steps (within doc_maintainer content boundaries)

1. In the `fdemon-app` module map / version-check description, note that the on-disk cache
   (`version_check.rs`) is keyed by the writing binary's version and stores the raw latest tag
   (re-compared at read time), so cross-version cache poisoning cannot suppress the banner.
2. In the data-flow / TUI rendering description, note that `startup_notice` is surfaced both above
   the New Session Dialog and as a top-row banner on the main/loading screens (so auto-launch
   sessions still see it), cleared on first keypress.
3. Keep edits to the appropriate sections only; do not add implementation detail beyond
   architecture-level data flow.

## Acceptance criteria

- [x] ARCHITECTURE.md describes version-keyed cache + decoupled banner render path.
- [x] No content-boundary violations (architecture-level only; no how-to/config detail — that
      lives in `docs/CONFIGURATION.md`, Task 04a).

---

## Completion Summary

**Status:** Done
**Branch:** fix/version-check-banner-not-appearing

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Updated `version_check.rs` module-table row to describe version-keyed cache (current_version field, cross-version poisoning prevention, raw-tag storage rationale). Updated `spawn_version_check` startup-sequence entry to document decoupled `startup_notice` render path (dialog + top-row banner on all other screens). Updated Internal key-types entry to note version-keyed cache. |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: N/A

### Notable Decisions/Tradeoffs

1. **Three targeted edits, not a rewrite**: Only the three existing entries that describe version-check behavior were modified. No new sections were added — the information fits naturally into the existing module table and data-flow startup sequence.
2. **No implementation detail**: Cache format fields and render logic are described at the architecture level (what is stored and why, which render locations exist) without reproducing code.
