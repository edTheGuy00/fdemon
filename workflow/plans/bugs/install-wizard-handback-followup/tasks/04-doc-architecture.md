## Task: Update ARCHITECTURE.md for the preflight SDK-resolution change (Finding 1 docs)

**Agent:** doc_maintainer

**Objective**: Reflect the `run_preflight` return-type change in `docs/ARCHITECTURE.md` — it now
returns the resolved `FlutterSdk` alongside the report so the `RunToolchainPreflight` executor no
longer re-resolves the SDK.

**Depends on**: 01-harden-handback-sdk-resolution

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**

- `docs/ARCHITECTURE.md` — targeted edit to the `toolchain/mod.rs` entry in the project-structure
  tree (the line describing `run_preflight`). Note that `run_preflight` now returns the resolved
  Flutter SDK with the report (e.g. a `PreflightOutcome { report, flutter_sdk }`) so the executor
  populates `resolved_sdk` from a single resolution — removing the former second `find_flutter_sdk`
  call and the report-Ok-but-SDK-unresolved gap. Match the existing terse parenthetical style.

**Files Read (Dependencies):**

- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules.
- The task 01 implementation files for the exact return type / symbol names.

### Acceptance Criteria

1. ARCHITECTURE.md accurately describes the new `run_preflight` return type and the
   single-resolution data flow.
2. No content-boundary violations; targeted edit only (no rewrite of unrelated sections).

### Notes

- Keep it to the `toolchain/mod.rs` (and, if relevant, `actions/mod.rs` executor) descriptions.
- `docs/KEYBINDINGS.md` is handled in task 02 (implementor-editable), not here.

---

## Completion Summary

**Status:** Not Started
**Branch:** <fill in>

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs
