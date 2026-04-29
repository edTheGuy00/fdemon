# Windows `flutter devices` Spawn Failure — Task Index

## Overview

Fix the Windows-only `flutter devices failed with exit code Some(1): The system cannot find the path specified.` startup error reported in issues #32 and #34, plus the latent locator bugs for shim-style Flutter installs. Add Windows CI so we don't ship this kind of regression again.

See `BUG.md` in this directory for the full root-cause analysis. User-confirmed design choices:

- **Keep `FlutterExecutable` as an enum** — both variants will end up calling `tokio::process::Command::new(path)` directly, but the enum is preserved for future metadata use and to avoid an API churn in `fdemon-daemon`.
- **Use `dunce::canonicalize`** to strip `\\?\` UNC prefixes that `cmd.exe` cannot handle.
- **Add Windows CI** via a new `.github/workflows/ci.yml`.

**Total Tasks:** 7 (6 implementor + 1 doc_maintainer)

## Task Dependency Graph

```
Wave 0 (parallel, no deps)
├── 01-add-windows-deps          (Cargo.toml + daemon Cargo.toml)
└── 06-add-windows-ci            (.github/workflows/ci.yml)

Wave 1 (parallel, all depend on 01)
├── 02-simplify-flutter-executable    (types.rs)
├── 03-locator-which-dunce            (locator.rs)
└── 04-diagnostic-error-paths         (devices.rs, process.rs, version_probe.rs)

Wave 2 (depends on 02 + 03)
└── 05-windows-only-tests       (NEW windows_tests.rs + mod.rs)

Wave 3 (depends on 02 + 03 + 04)
└── 07-update-docs              (ARCHITECTURE.md, DEVELOPMENT.md) — doc_maintainer
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules | Agent |
|---|------|--------|------------|------------|---------|-------|
| 1 | [01-add-windows-deps](tasks/01-add-windows-deps.md) | Done | — | 0.5h | `Cargo.toml`, `crates/fdemon-daemon/Cargo.toml` | implementor |
| 2 | [02-simplify-flutter-executable](tasks/02-simplify-flutter-executable.md) | Done (CONCERN: docs/DEVELOPMENT.md MSRV bump deferred to task 07) | 1 | 1-2h | `crates/fdemon-daemon/src/flutter_sdk/types.rs` | implementor |
| 3 | [03-locator-which-dunce](tasks/03-locator-which-dunce.md) | Done | 1 | 2-3h | `crates/fdemon-daemon/src/flutter_sdk/locator.rs` | implementor |
| 4 | [04-diagnostic-error-paths](tasks/04-diagnostic-error-paths.md) | Done | 1 | 1-2h | `crates/fdemon-daemon/src/devices.rs`, `crates/fdemon-daemon/src/process.rs`, `crates/fdemon-daemon/src/flutter_sdk/version_probe.rs` | implementor |
| 5 | [05-windows-only-tests](tasks/05-windows-only-tests.md) | Done (follow-up: removed unused PathBuf import) | 2, 3 | 2-3h | `crates/fdemon-daemon/src/flutter_sdk/windows_tests.rs` (NEW), `crates/fdemon-daemon/src/flutter_sdk/mod.rs` | implementor |
| 6 | [06-add-windows-ci](tasks/06-add-windows-ci.md) | Done | — | 1-2h | `.github/workflows/ci.yml` (NEW) | implementor |
| 7 | [07-update-docs](tasks/07-update-docs.md) | Done (also resolved task 02 MSRV carry-over) | 2, 3, 4 | 0.5-1h | `docs/ARCHITECTURE.md`, `docs/DEVELOPMENT.md` | doc_maintainer |

**Estimated total:** 8-13 hours

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-add-windows-deps | `Cargo.toml`, `crates/fdemon-daemon/Cargo.toml` | — |
| 02-simplify-flutter-executable | `crates/fdemon-daemon/src/flutter_sdk/types.rs` | `Cargo.toml` (for which version) |
| 03-locator-which-dunce | `crates/fdemon-daemon/src/flutter_sdk/locator.rs` | `crates/fdemon-daemon/src/flutter_sdk/types.rs`, `Cargo.toml` |
| 04-diagnostic-error-paths | `crates/fdemon-daemon/src/devices.rs`, `crates/fdemon-daemon/src/process.rs`, `crates/fdemon-daemon/src/flutter_sdk/version_probe.rs` | `crates/fdemon-daemon/src/flutter_sdk/types.rs` |
| 05-windows-only-tests | `crates/fdemon-daemon/src/flutter_sdk/windows_tests.rs` (new file), `crates/fdemon-daemon/src/flutter_sdk/mod.rs` | `crates/fdemon-daemon/src/flutter_sdk/types.rs`, `crates/fdemon-daemon/src/flutter_sdk/locator.rs` |
| 06-add-windows-ci | `.github/workflows/ci.yml` (new file) | — |
| 07-update-docs | `docs/ARCHITECTURE.md`, `docs/DEVELOPMENT.md` | All implementation files (read for change context) |

### Overlap Matrix

Compared pairwise within each wave (tasks that have no dependency between them).

**Wave 0 — `01` and `06`:**

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 06 | None | Parallel (worktree) |

**Wave 1 — `02`, `03`, `04`:**

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 02 + 03 | None (`types.rs` vs `locator.rs`) | Parallel (worktree) |
| 02 + 04 | None (`types.rs` vs `devices.rs`/`process.rs`/`version_probe.rs`) | Parallel (worktree) |
| 03 + 04 | None (`locator.rs` vs `devices.rs`/`process.rs`/`version_probe.rs`) | Parallel (worktree) |

**Cross-wave:** Tasks in later waves depend on earlier waves and run after them — no overlap concerns.

**Note on Wave 2 isolation:** Task `05` adds a brand-new `windows_tests.rs` file (no overlap with 02 or 03's writes) and only adds *one line* (`#[cfg(target_os="windows")] mod windows_tests;`) to `flutter_sdk/mod.rs`, which is not modified by any other task. Safe to run alone.

## Success Criteria

The bug is fixed when:

- [ ] `cargo build --target x86_64-pc-windows-msvc` succeeds (cross-compile from macOS or via the new CI runner).
- [ ] On a Windows host with Flutter on PATH, `fdemon` discovers devices without the `The system cannot find the path specified` error. **Validation surrogate (since we don't have a Windows machine):** the new Windows-only unit tests (task 05) all pass on the `windows-latest` CI runner.
- [ ] `which::which("flutter")` is the primary discovery mechanism and resolves to the absolute `flutter.bat` path (with `.bat` extension) on Windows; `flutter` (no extension) on Unix.
- [ ] `dunce::canonicalize` is used everywhere `fs::canonicalize` was previously used in the locator, to avoid `\\?\` UNC prefixes leaking into `cmd.exe`.
- [ ] `FlutterExecutable::command()` returns `Command::new(path)` for both variants — no manual `cmd /c` wrapper.
- [ ] On non-zero exit from `flutter devices`, the user-facing log includes the resolved binary path, full command line, and complete stderr (not just the first line).
- [ ] `.github/workflows/ci.yml` exists and runs `fmt + check + test + clippy` on `ubuntu-latest`, `macos-latest`, and `windows-latest`.
- [ ] `cargo fmt && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes on all three platforms (CI green).
- [ ] `docs/ARCHITECTURE.md` and `docs/DEVELOPMENT.md` reflect the new dependencies and CI.
- [ ] Issues #32 and #34 receive a comment with a Windows test build link and a request for verification + log files.

## Notes

- **No Windows machine available locally.** All Windows verification runs in GitHub Actions. Each task's acceptance criteria explicitly call out which checks are CI-only.
- **`adb` is fine as-is.** `Command::new("adb")` works on Windows because `adb.exe` is a real PE binary that `CreateProcessW` resolves via standard `.exe` lookup. No change needed in `crates/fdemon-daemon/src/native_logs/android.rs`.
- **`fdemon-daemon/src/native_logs/` has no Windows backend.** This is a separate gap — out of scope for this bug. File a follow-up issue.
- **The locator's strategies 1-9 (FVM, Puro, asdf, mise, proto, etc.) are not changed** in this PR. They use the same `validate_sdk_path` path; once we drop `WindowsBatch` it's a transparent improvement.
- **Argument-escaping for user-supplied dart-defines** (a post-CVE-2024-24576 concern) is **out of scope** for this fix. We will surface a clear error if `Command::spawn()` returns `Err(InvalidInput)` on Windows but we are not pre-validating dart-define values. Track separately.
- **Issue follow-up (commenting on #32 and #34)** is a manual human task once the binary is built and uploaded as a CI artifact. Not represented as an implementor task in this list.
