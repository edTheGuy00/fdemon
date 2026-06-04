# Task 03 — Harden `path_config.rs`: PowerShell + shell injection

**Agent:** implementor
**Status:** Not Started
**Depends On:** -
**Estimated Hours:** 3-4h
**Module:** `crates/fdemon-daemon/src/toolchain/path_config.rs`

## Context

`path_config.rs` writes the Flutter `bin` dir onto the user's `PATH` — into shell rc files
(POSIX) or the Windows user registry via PowerShell. Because these writes are later
*executed* by the shell, untrusted path content is an injection vector. The Phase 2 review
found a **CRITICAL PowerShell code-injection** and a **MAJOR POSIX/fish shell-injection**,
plus three minors. All live in this one file.

References: `workflow/reviews/features/toolchain-bootstrap-phase-2/ACTION_ITEMS.md`
(C2, M10, m2, m5, m8) and `REVIEW.md`.

## Findings to Fix

### C2 — PowerShell code injection (CRITICAL) — `add_to_path_windows`, ~line 332-340
The new PATH is interpolated into a `-Command` string and "escaped" with
`new_path.replace('\'', "\\'")`. PowerShell single-quote escaping is **doubling** (`''`),
not backslash; backtick (`` ` ``) and `$(...)` remain live. A path containing PS
metacharacters executes arbitrary code as the user. This path has **no runtime test**.

**Fix:** Stop interpolating the value into the script. Pass it out-of-band via the
process environment and reference it inside the script:

```text
Command::new("powershell")
    .args(["-NoProfile", "-NonInteractive", "-Command",
           "[Environment]::SetEnvironmentVariable('PATH', $env:FDEMON_NEW_PATH, 'User')"])
    .env("FDEMON_NEW_PATH", &new_path)
    .output()
```

This removes the injection surface entirely. (The read step that fetches the current PATH
is already a constant script with no interpolation — leave it, but you may apply the same
env-passing discipline for consistency.)

### M10 — POSIX/fish rc-file shell injection (MAJOR security) — `posix_export_line`/`fish_add_path_line`, ~line 123-130
The export line is written verbatim into `.bashrc`/`.zshenv`/`config.fish`. A `bin_dir`
containing a newline followed by a command (e.g. from a repo-checked-in
`.fdemon/config.toml` → `[toolchain] flutter_install_dir`, or `$FVM_CACHE_PATH`) poisons
the shell config and executes on next shell start. `fish_add_path` is unquoted, so spaces
and metacharacters also break it.

**Fix:**
- Add `fn validate_bin_dir(bin_dir: &Path) -> Result<()>` (called at the top of
  `add_to_path`, before any write, for all platforms) that rejects paths containing
  newline (`\n`/`\r`) and shell control metacharacters (`` ` ``, `$(`, `;`, `&`, `|`,
  `\n`). Return a clear `Error::config` naming the rejected character class.
- Single-quote and escape the fish argument: `fish_add_path '<escaped>'` where embedded
  `'` becomes `'\''` (POSIX single-quote escaping).
- For the bash/zsh `export PATH="$PATH:<bin>"` line, the path is already inside double
  quotes; combined with the newline/metachar rejection above this closes the injection.
  Consider escaping `"`, `` ` ``, `$`, `\` within the double-quoted segment defensively.

### m2 — `home_dir()` cfg fragility (MINOR) — ~line 361-370
Uses `#[cfg(not(target_os = "windows"))]` / `#[cfg(target_os = "windows")]` string
comparisons. Replace with the idiomatic `#[cfg(windows)]` / `#[cfg(not(windows))]`.

### m5 — macOS bash login-shell gap (MINOR) — `rc_file_for_shell`, ~line 70-84
bash on macOS sources `.bash_profile`/`.profile` for login shells, not `.bashrc`, so the
PATH write can silently appear to "not work."

**Fix:** For `HostShell::Bash`, when the platform is macOS prefer `.bash_profile` if it
exists (falling back to `.profile`, then `.bashrc`); on Linux keep `.bashrc`. Keep the
returned rc-file path surfaced to the caller so the wizard can show *which* file was
written (it already returns `rc_file` in `PathConfigOutcome::Written`).

### m8 — Swallowed `remove_file` error without trace (MINOR) — `write_rc_atomically`, ~line 255-257
The best-effort temp-file cleanup on rename failure (`let _ = std::fs::remove_file(...)`)
should `tracing::debug!` the failure (consistent with `flutter_install.rs`'s cleanup logs).

## Acceptance Criteria

- [ ] Windows PATH set passes the value via `FDEMON_NEW_PATH` env, not interpolation; a
      Windows-gated unit test (or a string-construction test asserting no interpolation of
      the path into the `-Command` arg) covers a path containing a space and a `'`.
- [ ] `validate_bin_dir` rejects newline/metacharacter paths before any write; tests cover
      accept + reject for each rejected class.
- [ ] `fish_add_path` argument is single-quoted/escaped; `test_fish_uses_fish_add_path`
      updated to assert quoting. POSIX export idempotency tests still pass.
- [ ] `home_dir()` uses `#[cfg(windows)]`/`#[cfg(not(windows))]`.
- [ ] bash on macOS targets `.bash_profile`/`.profile` appropriately (test with a temp
      HOME containing/omitting those files); Linux bash still uses `.bashrc`.
- [ ] Temp-file cleanup failure is `debug!`-logged.
- [ ] All existing `path_config.rs` golden-file/idempotency/rc-selection tests pass; new
      tests added for the above.
- [ ] Only `path_config.rs` is modified. `cargo fmt`/`check`/`test -p fdemon-daemon`/
      `clippy -D warnings` pass.

## Notes

- `validate_bin_dir` is the single chokepoint — call it once at the top of `add_to_path`
  so both the Windows and POSIX paths are covered.
- Reuse workspace `Error`/`Result`; no `unwrap()` in non-test code.
- The `#[cfg(test)]`-only `fence_already_has_dir` helper may be left as-is or unified with
  the inline check in `apply_fence` — optional cleanup, not required.
