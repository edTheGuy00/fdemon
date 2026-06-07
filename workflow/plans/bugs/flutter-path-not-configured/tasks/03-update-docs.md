# Task 03 — Document the auto-PATH-config chain

**Agent:** doc_maintainer
**Severity:** 🟡 MINOR (docs)
**Depends On:** 01, 02
**Crate(s):** docs

## Goal

Update `docs/ARCHITECTURE.md` to reflect the behavioural changes from tasks 01–02.
Stay strictly within ARCHITECTURE.md content boundaries (module behaviour, data
flow, invariants) — no changelog entries, no build/test commands.

## Required Updates

- **Install-wizard data flow:** a successful `FlutterSdk` / `AndroidTools` install
  now **auto-configures PATH** (auto-runs the PathConfig write — Flutter `<sdk>/bin`
  and, for Android, `ANDROID_HOME` + Android `PATH`) before/with the preflight
  re-check, instead of requiring a manual PathConfig step. Document the chain
  (completion → persist settings → auto PathConfig → re-run preflight) and the
  seq-guard / no-loop invariant.
- If task 01 added a state field (e.g. `pending_path_autoconfig`) or a new
  `Message` variant, reflect it in the `install_wizard/` state and Message-category
  descriptions.
- **rc-writer test seam:** note that `path_config.rs` home resolution has a
  test-only override seam so the suite never writes to a real `~/.zshenv`
  (one line in the `toolchain/path_config.rs` description).

## Files Modified (Write)

- `docs/ARCHITECTURE.md`

## Acceptance Criteria

- [ ] ARCHITECTURE.md describes the auto-PATH-config chain and its no-loop / seq-guard
      invariant.
- [ ] New state field / message variant (if any) from task 01 is documented.
- [ ] rc-writer test seam noted.
- [ ] No content-boundary violations (no changelog/build/test content).
