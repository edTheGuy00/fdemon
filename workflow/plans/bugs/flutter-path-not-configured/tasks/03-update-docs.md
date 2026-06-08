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

- [x] ARCHITECTURE.md describes the auto-PATH-config chain and its no-loop / seq-guard
      invariant.
- [x] New state field / message variant (if any) from task 01 is documented.
- [x] rc-writer test seam noted.
- [x] No content-boundary violations (no changelog/build/test content).

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | (1) Added `InstallWizardAutoConfigurePath { kind }` to Message Categories with full routing description and no-loop/seq-guard invariant. (2) Extended `handler/install_wizard/` module description with `handle_auto_configure_path` chain for FlutterSdk- and AndroidTools-origin auto-configs. (3) Updated Install Wizard Step Execution Flow diagram with the new completion → persist → auto-PathConfig → re-run-preflight chains for both FlutterSdk and AndroidTools. (4) Added one-line note on the test-only `home_dir()` thread-local override in `toolchain/path_config.rs`. |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: N/A

### Notable Decisions/Tradeoffs

1. **Flow diagram placement**: Added the `WizardStepCompleted(AndroidTools)` block as a sibling to the existing `WizardStepCompleted(FlutterSdk)` block in the diagram, rather than a sub-branch, so both origins and their differing `android_sdk_root` behaviour are equally visible.
2. **"← NEW" annotation**: Used inline `← NEW (auto-PATH-config)` labels in the diagram to make the additions scannable without altering the surrounding established notation style.
