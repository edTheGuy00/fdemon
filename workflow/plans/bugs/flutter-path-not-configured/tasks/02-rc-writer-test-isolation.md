# Task 02 — Fence rc-file writers off from the real `$HOME` in tests + temp-dir hygiene

**Agent:** implementor
**Severity:** 🟠 MAJOR (prevents tooling from corrupting a developer's real shell config)
**Depends On:** —
**Crate(s):** `fdemon-daemon`

## Problem

The public rc-file writers resolve the **real** `$HOME` and write to the real
`~/.zshenv` / `~/.zprofile`:

- `add_to_path` → `home_dir()` → `rc_file_for_shell` → real rc file
  (`crates/fdemon-daemon/src/toolchain/path_config.rs:217-238`, `159-192`).
- `add_android_env` likewise (`path_config.rs:281-303`).

Today's two tests that call these (`test_add_to_path_rejects_injection_path`
:1550, `test_add_android_env_rejects_injection_path` :2202) pass newline-bearing
paths that `validate_bin_dir` rejects **before** any I/O, so they are safe. But the
seam is fragile: any test calling the public writers with a **clean** path and a
supported shell on a matching platform would append a fence block to the
developer's real `~/.zshenv`. This is the most plausible origin of the reported
stale `/tmp/.tmp…/bin` artifact (a `tempfile::TempDir` path — production toolchain
code never uses `tempfile`). A leftover empty Android SDK temp dir
(`/tmp/.tmpGOfMr6/`) was also observed.

## Goal

Make it **structurally impossible** for the test suite to mutate a developer's real
shell rc files, and confirm Android-install temp dirs never leak onto the real
filesystem.

## Acceptance Criteria

- [ ] Home resolution for the rc-file writers goes through an **injectable seam**
      (e.g. `home_dir()` honours a test-only override, or test-only writer variants
      that accept an explicit `home: &Path`). All `path_config.rs` tests exercising
      the real writers use a `TempDir` home — none touch `$HOME`-derived paths.
- [ ] Audit is workspace-wide: confirm no test in any crate reaches a real-`$HOME`
      rc-file write via `add_to_path` / `add_android_env` / `home_dir()`. Cite the
      audit result in the completion summary.
- [ ] A **regression guard** test fails if a clean path is ever written through the
      public writers against an unsandboxed home (e.g. assert the seam is active, or
      assert the writers refuse a non-overridden home in `cfg(test)`).
- [ ] Android-install temp handling verified: tests use `TempDir` (auto-removed on
      drop); `relocate_cmdline_tools` / the android temp flow never leaves an empty
      `sdk_root` on the real FS. Add/adjust a test asserting cleanup.
- [ ] Existing `path_config.rs` and `android_install.rs` tests still pass; the
      injection-rejection tests still reject before I/O.

## Recommended Approach

- Prefer the already-present explicit-path helpers in tests:
  `add_to_rc_file(rc_file, bin)`, `add_android_env_to_rc_file(rc_file, sdk_root)`,
  and `rc_file_for_shell(shell, &temp_home)` — these take an explicit path and never
  resolve `$HOME`. Reserve the `home_dir()`-resolving public functions for the
  error-path tests that reject **before** I/O.
- For the injectable seam, a small `home_dir()` that checks a `#[cfg(test)]`
  thread-local / atomic override (or an env override honoured only under `cfg(test)`)
  is sufficient; document it. Keep production behaviour identical (real `$HOME`).
- Keep the change scoped to `fdemon-daemon`; do not alter the public signatures used
  by `fdemon-app`'s executor (`actions/mod.rs` calls `add_to_path(shell, platform,
  bin)` / `add_android_env(shell, platform, sdk_root)`).

## Files Modified (Write)

- `crates/fdemon-daemon/src/toolchain/path_config.rs`
- `crates/fdemon-daemon/src/toolchain/android_install.rs`

## Files Read (Dependencies)

- `crates/fdemon-app/src/actions/mod.rs` (confirm public writer call sites are
  unaffected — read only)

## Testing

- New regression guard test (as above).
- Run the full `path_config` and `android_install` test modules; confirm no real
  `~/.zshenv` write occurs (e.g. by asserting the seam override is required under
  `cfg(test)`).
- `cargo fmt`, `cargo check`, `cargo test`, `cargo clippy -D warnings` all green.

## Notes

- Do not change production home resolution semantics — only add a test-only override
  seam. The reported artifact is from an older build; this task prevents recurrence.
- If the audit finds a currently-offending test, fix it as part of this task and
  note it explicitly.
