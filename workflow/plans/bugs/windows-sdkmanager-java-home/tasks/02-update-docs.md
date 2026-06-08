# Task 02 — Document the Android installer's JDK-home fallback + validation

**Agent:** doc_maintainer
**Severity:** 🟡 MINOR (docs)
**Depends On:** 01
**Crate(s):** docs

## Goal

Update `docs/ARCHITECTURE.md` to reflect task 01: the Android installer
(`toolchain/android_install.rs`) now resolves the JDK home for the `sdkmanager`
child via `target.jdk_path` → `resolve_jdk_home()` (the missing fallback), validates
it (exists + `bin/java` + `bin/javac` — a true JDK, normalized, no trailing
separator), and always sets `JAVA_HOME` + prepends `<home>/bin` to the child PATH.
If no valid JDK home resolves, the step fails with an actionable error. Also note the
pre-spawn `sdkmanager` existence guard. Stay within ARCHITECTURE.md content
boundaries (module behaviour/data flow — no changelog/build/test content, no code
samples).

## Required Updates

- `toolchain/android_install.rs` description: JDK-home precedence + validation, the
  always-set `JAVA_HOME`/child-PATH behaviour, and the fail-with-guidance path.
- `toolchain/jdk.rs` description: note the new `validate_jdk_home` helper (true-JDK
  check, Windows `.exe` suffixes) used by the installer.

## Files Modified (Write)

- `docs/ARCHITECTURE.md`

## Acceptance Criteria

- [x] ARCHITECTURE.md describes the installer's JDK-home fallback to
      `resolve_jdk_home()` and the validation (true JDK, normalized).
- [x] The `validate_jdk_home` helper is noted in the `jdk.rs` description.
- [x] No content-boundary violations.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Updated `toolchain/android_install.rs` row: added `check_sdkmanager_guard` pre-spawn guard, `build_sdkmanager_env` JDK-home resolution (target.jdk_path → resolve_jdk_home() fallback), `validate_jdk_home` validation, always-set JAVA_HOME + child PATH prepend, and fail-with-actionable-error path. Updated `toolchain/jdk.rs` row: added `validate_jdk_home` helper description (path normalisation, true-JDK check, Windows .exe suffixes). |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: N/A

### Notable Decisions/Tradeoffs

1. **Prose depth vs. boundary rules**: The new android_install.rs description is detailed but remains within ARCHITECTURE.md's "module behaviour / data flow" scope — it describes what the module does and why (the Windows JAVA_HOME problem), not how the code is written or how to build/run it.
