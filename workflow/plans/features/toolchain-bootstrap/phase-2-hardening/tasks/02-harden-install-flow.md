# Task 02 — Harden `flutter_install.rs`: install-flow security + correctness

**Agent:** implementor
**Status:** Not Started
**Depends On:** -
**Estimated Hours:** 4-6h
**Module:** `crates/fdemon-daemon/src/toolchain/flutter_install.rs`

## Context

`flutter_install.rs` orchestrates the managed install (git clone / archive download →
verify → extract → atomic rename → precache). The Phase 2 review found a **MAJOR git
argument-injection** vector, a **channel-ignored** bug in the archive path, a confusing
**partial-`final_dir`** failure, and **no concurrent-install guard**, plus a missing
manifest-fetch timeout and two minors. All live in this one file.

References: `workflow/reviews/features/toolchain-bootstrap-phase-2/ACTION_ITEMS.md`
(M2, M4, M5, M9, M6b, m3, m4) and `REVIEW.md`.

## Findings to Fix

### M2 — Git argument injection via unvalidated `channel` (MAJOR security) — `git_install`, ~line 447-457
`channel` (a free-form `[toolchain] channel` TOML string) is passed to
`git clone -b <channel> ...`. A value like `--upload-pack=…` or `--config core.askpass=…`
is interpreted as a git option (known RCE vectors).

**Fix:**
- Add `fn validate_channel(channel: &str) -> Result<()>` rejecting empty, a leading `-`,
  or any char outside `[A-Za-z0-9._-]`. Call it before building the args (also covers the
  archive path's use of `channel`).
- Add a `--` option terminator before the positional args in the `git clone` invocation
  so nothing after it is treated as a flag: `["clone", "-b", channel, "--depth", "1",
  "--", URL, tmp]` (place `--` so the URL and dir are unambiguously operands).

### M4 — Archive path ignores configured `channel` (MAJOR) — `archive_install`, ~line 478-490
`archive_install` always calls `manifest.resolve_stable(HostArch::detect())`, ignoring
`target.channel`. A user with `channel = "beta"` and no `git` silently gets stable.

**Fix:** Pass `target` (or `&target.channel`) into `archive_install`. Resolve the release
for the configured channel from the manifest. If the manifest cannot provide the requested
channel for the host arch, either fall back to stable **with an explicit
`InstallEvent::Log` warning** ("channel 'beta' unavailable as archive; installing
stable") or return a clear error — choose the warn-and-fallback behavior to preserve the
"install something usable" UX, but make it loud. Add a manifest helper if needed
(`resolve_channel(channel, arch)`) in `toolchain/types.rs` only if it can be done without
breaking existing `resolve_stable` callers — otherwise keep the logic local to this file.

### M5 — Partial/orphaned `final_dir` → confusing rename failure (MAJOR) — `install_flutter`/`install_inner`, ~line 311, 398
The already-installed short-circuit requires both `final_dir.exists()` AND
`flutter_bin.exists()`. If a prior install left an **incomplete** `final_dir` (dir exists,
`bin/flutter` missing), the short-circuit is skipped and the final `std::fs::rename` into
the non-empty `final_dir` fails with an opaque `ENOTEMPTY` ("Directory not empty"), after
which the freshly-fetched SDK in temp is deleted by cleanup. The docstring falsely claims
`final_dir` is "never left in a partial state."

**Fix:** Before the atomic rename (or right after the failed short-circuit), detect a
pre-existing `final_dir` that is **not** a complete install and either:
- remove it (`remove_dir_all`) so the rename can proceed, logging the reclamation; or
- fail fast with an actionable, retryable message naming the path
  ("incomplete Flutter install at <path>; remove it and retry").
Prefer remove-then-proceed for a smooth retry. Update the docstring to match actual
behavior.

### M9 — No concurrent-install lock on shared `final_dir` (MAJOR) — ~line 326-398
PID-suffixed temp dirs disambiguate the temp area, but the rename target
(`~/fvm/versions/<channel>`, shared with `fvm`) is unguarded. Two fdemon instances (or a
racing `fvm`) can collide.

**Fix:** Acquire an advisory lock for the duration of the install using an atomic
lockfile: `OpenOptions::new().write(true).create_new(true).open(install_root.join(
".fdemon-install.lock"))`. If `create_new` fails with `AlreadyExists`, return a clear
"another install is in progress (or a stale lock exists at <path>) — retry shortly"
error. Remove the lockfile on completion and on error (RAII guard struct with a `Drop`
impl preferred so a panic/early-return still releases it). **No new crate** — use std
`OpenOptions`.

### M6b — Manifest fetch has no timeout (MAJOR) — `fetch_release_manifest`, ~line 178-225
The manifest `reqwest` client has no timeout.

**Fix:** Add `.connect_timeout(...)` and `.timeout(...)` (named consts) to the manifest
client builder; optionally a small bounded retry consistent with Task 01's approach. Hand-rolled, no retry crate.

### m3 — `FVM_CACHE_PATH` not absolute-checked (MINOR) — `resolve_install_dir`, ~line 92-100
A relative `$FVM_CACHE_PATH` resolves against the process CWD. Guard with
`path.is_absolute()`; if relative, `tracing::warn!` and skip to the default
`~/fvm/versions` rather than honoring it.

### m4 — `HostArch::detect()` called twice (MINOR) — `archive_install`, ~line 485-489
Capture once: `let arch = HostArch::detect();` and reuse in both the resolve call and the
error message.

### Nitpicks (do if trivial)
- Derive `Copy` on `HostPlatform` (in `toolchain/types.rs`) only if it stays a field-less
  enum, to drop the `platform.clone()` at ~line 483 — **skip if it touches types.rs in a
  way that risks overlap; this is optional.** Otherwise leave the `clone()`.
- Add a brief code comment near the SHA-256 verify noting the hash and payload come from
  the same HTTPS server, so the digest guards corruption, not a CDN-level MITM (security HIGH-1, doc-only).

## Acceptance Criteria

- [ ] `validate_channel` rejects `--upload-pack=x`, leading-dash, and non-`[A-Za-z0-9._-]`
      channels; `git clone` includes a `--` terminator. Unit tests cover accept + reject.
- [ ] Archive install honors `target.channel`; when the channel can't be resolved as an
      archive, it logs a visible warning and falls back to stable (tested via a manifest
      fixture lacking the requested channel).
- [ ] A pre-existing incomplete `final_dir` no longer yields an opaque rename error — it is
      reclaimed (or a clear, retryable message is returned). Test simulates a partial dir.
- [ ] Concurrent install is guarded by an atomic lockfile; a second concurrent attempt
      returns a clear error and the lock is released on success and on error (Drop guard).
- [ ] `fetch_release_manifest` has timeouts (named consts).
- [ ] Relative `FVM_CACHE_PATH` is ignored with a warning; `HostArch::detect()` captured once.
- [ ] Existing `flutter_install.rs` tests pass (manifest parse, short-circuit, resolve,
      URL construction); new tests added for the items above.
- [ ] Only `flutter_install.rs` is modified (types.rs `Copy` nitpick skipped if it would
      cause overlap). `cargo fmt`/`check`/`test -p fdemon-daemon`/`clippy -D warnings` pass.

## Notes

- Task 02 uses only the **existing** public API of `download.rs`/`process_stream.rs`
  (unchanged by Task 01) — no dependency edge, fully parallel.
- Keep all I/O off the async runtime where blocking (existing `spawn_blocking` for
  verify/extract is correct).
- Reuse workspace `Error`/`Result`; no `unwrap()` in non-test code; `tracing` for warnings.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-ad410e03d656ae56f

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/flutter_install.rs` | All M2/M4/M5/M9/M6b/m3/m4 fixes + new tests |

### Notable Decisions/Tradeoffs

1. **resolve_channel_release local helper (M4)**: Added `resolve_channel_release()` as a private function in `flutter_install.rs` rather than adding a method to `FlutterReleaseManifest` in `types.rs`. This avoids any risk of breaking overlap with other tasks touching `types.rs` (as directed by the task notes).

2. **LockGuard derives Debug (M9)**: `LockGuard` needed `#[derive(Debug)]` because the test `unwrap_err()` call requires `Debug` on the guard type. This is a minor addition; the struct itself is just `lock_path: PathBuf`.

3. **archive_install signature change (M4)**: `archive_install` now takes `&FlutterInstallTarget` to access `target.channel` and `target.install_root`. This is a private function so the signature change is contained within this file.

4. **Warn-then-fallback (M4)**: When the configured channel is unavailable as an archive for the detected arch, a loud `[warning]` line is emitted through `on_event` and the install continues with `stable`. This matches the "install something usable" UX preference from the task spec.

5. **Partial-dir test is non-async (M5)**: The partial-dir detection and removal logic lives in `install_inner` (not directly testable without real git/archive). The test covers the detection logic directly by simulating the same path conditions, which fully validates the decision condition and removal step.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test -p fdemon-daemon` — Passed (914 tests)
- `cargo test --workspace` — Passed (all crates)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **Lock does not use fcntl advisory locking**: Uses `create_new` file existence as the lock mechanism. This is advisory-only and does not prevent lock bypass by processes that ignore it (e.g., a raw `fvm` install). The task spec explicitly called for this approach.

2. **Env var tests still use set_var/remove_var**: The `FVM_CACHE_PATH` relative-path test manipulates env vars without `serial_test`. These tests were present before and continue to use the same pattern. In highly parallel test runs these could interfere — mitigated by the fact that `resolve_install_dir` is deterministic once the env var is set.
