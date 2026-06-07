# Windows sdkmanager JAVA_HOME — Task Index

## Overview

Fix the confirmed Windows bug where the Android Tools step fails at license
acceptance ("sdkmanager … The system cannot find the path specified") because
`sdkmanager.bat` is handed a broken/absent ambient `JAVA_HOME`. fdemon only sets
`JAVA_HOME` for the sdkmanager child when `[toolchain] jdk_path` is explicitly
configured; it must instead fall back to its existing `resolve_jdk_home()` and
validate the result. See [`BUG.md`](BUG.md).

Approved decisions: **require `bin/javac` (true JDK)** for validation; **fail the
step with guidance** when no valid JDK home resolves.

## Finding → Task Map

| Finding | Sev | Area | Task |
|---|---|---|---|
| Installer only sets JAVA_HOME when `jdk_path` is Some; no `resolve_jdk_home()` fallback | MAJOR | windows / android install | 01 |
| No JDK-home validation/normalization → cryptic "path specified" | MAJOR | robustness | 01 |
| No pre-spawn sdkmanager existence guard | MINOR | diagnostics | 01 |
| Docs must describe the JDK-home fallback + validation | — | docs | 02 |

## Tasks

| # | Task | Status | Depends On | Sev | Crate | Files Modified (Write) |
|---|------|--------|------------|-----|-------|------------------------|
| 01 | [01-android-sdkmanager-java-home](tasks/01-android-sdkmanager-java-home.md) | ✅ Done | — | MAJOR | fdemon-daemon | `toolchain/android_install.rs`, `toolchain/jdk.rs` |
| 02 | [02-update-docs](tasks/02-update-docs.md) | ✅ Done | 01 | MINOR | docs | `docs/ARCHITECTURE.md` |

## Task Dependency Graph

```
01 android JAVA_HOME fallback + validation (fdemon-daemon) ──▶ 02 docs (doc_maintainer)
```

## File Overlap Analysis

| Task | Files Modified (Write) |
|------|------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/android_install.rs`, `crates/fdemon-daemon/src/toolchain/jdk.rs` |
| 02 | `docs/ARCHITECTURE.md` |

### Overlap Matrix (write-file conflicts)

| Pair | Shared write files | Strategy |
|------|--------------------|----------|
| 01 ↔ 02 | none | **Sequential** — 02 depends on 01 (docs after impl) |

Two tasks in a dependency chain → no parallelism. 01 runs on the current branch
(single task), then 02 (docs).

## Suggested Wave Schedule

- **Wave 1:** 01 (single task, current branch)
- **Wave 2:** 02 docs (after 01) → `doc_maintainer`

## Success Criteria

- [ ] On Windows, Android Tools license acceptance succeeds when a JDK is installed,
      **without** manually setting `[toolchain] jdk_path`.
- [ ] The sdkmanager child always gets a validated `JAVA_HOME` + JDK `bin` on PATH
      (`target.jdk_path` → `resolve_jdk_home()`); a missing/invalid JDK yields an
      actionable error, not "The system cannot find the path specified".
- [ ] Pre-spawn `sdkmanager` existence guard with a directory-listing error on miss.
- [ ] POSIX behaviour unchanged; no regression.
- [ ] `docs/ARCHITECTURE.md` documents the fallback + validation.
- [ ] `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      pass; **E2E verified in the Windows VM** (`tests/docker/windows/`).

## Notes

- Reuses the existing `resolve_jdk_home()` (jdk.rs:30) — no new resolution logic;
  mirrors the PathConfig "fall back to the resolver when settings is None" pattern.
- Complements the just-merged Windows preflight PATH-refresh: `resolve_jdk_home()`'s
  `which java` now sees a JDK installed after fdemon launched.
- Authoritative verification is the real Windows 11 VM at `tests/docker/windows/`
  (rebuild `fdemon.exe`, re-stage, run the Android Tools step).
