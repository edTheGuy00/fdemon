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

- [ ] ARCHITECTURE.md describes version-keyed cache + decoupled banner render path.
- [ ] No content-boundary violations (architecture-level only; no how-to/config detail — that
      lives in `docs/CONFIGURATION.md`, Task 04a).
