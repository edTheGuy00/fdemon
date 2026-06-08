# Task 02 — Document the Windows preflight PATH-refresh

**Agent:** doc_maintainer
**Severity:** 🟡 MINOR (docs)
**Depends On:** 01
**Crate(s):** docs

## Goal

Update `docs/ARCHITECTURE.md` to reflect task 01: on Windows, `run_preflight`
refreshes the process `PATH` from the registry (expanded Machine + User `Path`)
before probing, so a wizard re-check (`r`) detects tools installed after fdemon
launched (e.g. git via winget). Note it is `#[cfg(windows)]`-only and a no-op
elsewhere, and that it complements the existing `WM_SETTINGCHANGE` broadcast on
write (write broadcasts to *other* processes; this re-reads into *fdemon's own*
process). Stay within ARCHITECTURE.md content boundaries (no changelog/build/test
content).

## Required Updates

- `toolchain/mod.rs` / `run_preflight` description: add the Windows PATH-refresh
  step and its rationale (running process keeps a frozen env block; registry PATH
  changes from installers are invisible until re-read).
- `toolchain/path_config.rs` description: note the new `#[cfg(windows)]`
  `refresh_process_path_from_registry()` helper alongside the existing write +
  broadcast helpers.

## Files Modified (Write)

- `docs/ARCHITECTURE.md`

## Acceptance Criteria

- [x] ARCHITECTURE.md describes the Windows-only preflight PATH-refresh and why it's
      needed, and that it's a no-op on non-Windows.
- [x] The new helper is noted in the `path_config.rs` description.
- [x] No content-boundary violations.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Four targeted edits: (1) project-tree `toolchain/mod.rs` line — added Windows PATH-refresh note and cfg(windows)-only / no-op caveat; (2) project-tree `toolchain/path_config.rs` line — noted new `refresh_process_path_from_registry()` helper; (3) Module Reference `toolchain/mod.rs` row — added rationale (frozen env block, registry re-read before probe fan-out, complement to WM_SETTINGCHANGE); (4) Module Reference `toolchain/path_config.rs` row — described helper, `merge_machine_user_path()` pure combiner, PowerShell out-of-band pattern, and complement relationship with broadcast. |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: N/A

### Notable Decisions/Tradeoffs

1. **Four edit sites, not two**: Both the project-structure tree and the Module Reference table carry descriptions of each file. Both needed updating for consistency — the tree entry is a one-liner summary; the table row is the authoritative prose. Updating only one would leave the document inconsistent.
2. **`merge_machine_user_path()` named explicitly**: Including the pure combiner helper in the `path_config.rs` description preserves the architectural distinction (pure function vs. side-effecting `set_var` call) without adding code samples.
