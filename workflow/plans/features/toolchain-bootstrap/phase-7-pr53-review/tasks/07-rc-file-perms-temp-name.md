## Task: Preserve rc-file permissions on atomic write and avoid the fixed temp-file name (F-PR53-13)

**Severity:** MEDIUM (security / concurrency)

**Objective**: Stop `write_rc_atomically` from silently downgrading the
permissions of a hardened (e.g. `chmod 600`) shell rc file, and remove the
deterministic temp-file name that lets concurrent writers clobber each other.

**Depends on**: 02 (shares `path_config.rs`)

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/path_config.rs`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/path_config.rs` (`add_to_rc_file`, `add_android_env_to_rc_file`, `read_rc_contents`, `rc_file_for_shell`)
- `Cargo.toml` (`tempfile` is already a dev-dependency; promote if used in non-test code)

### Details

**(a) Permission downgrade.**
`write_rc_atomically` (`path_config.rs:519-554`) creates a brand-new temp file via
`std::fs::write(&tmp_path, ...)` (line 528, honors umask → typically 0644) then
`std::fs::rename(&tmp_path, rc_file)` (line 536), replacing the destination inode.
No `set_permissions`/`metadata`/`mode()` anywhere in the file, so a
`chmod 600 ~/.zshenv` (which can hold `export FOO=secret`) is silently downgraded
to group/world-readable after fdemon edits it. Targets include `.zshenv`,
`.zprofile`, `.bashrc`, `.bash_profile`, `.profile` (`rc_file_for_shell`, 148-186).

**(b) Fixed temp-file name + unlocked read-modify-write.**
The temp path is deterministic: `rc_file.with_extension("fdemon_tmp")`
(line 526). Two concurrent fdemon processes targeting the same rc file derive the
identical temp path and the read→apply→write sequence (`read_rc_contents` →
`write_rc_atomically`, 557-571) is unlocked, so a lost-update / temp-clobber
window exists across processes (low-likelihood, single-user TUI, but the fixed
name removes any safety margin).

### Proposed Fix

1. Before renaming, if the destination exists, read its metadata
   (`std::fs::metadata(rc_file)`) and apply the original mode to the temp file via
   `std::fs::set_permissions` (Unix: copy the full `mode()`); if the file does not
   exist, restrict the new file to `0600`. (Best effort on non-Unix.)
2. Use a unique temp file name — prefer `tempfile::NamedTempFile::new_in(parent)`
   (already a dev-dep; promote to a normal dep) or include the PID + a nonce in the
   suffix — so concurrent writers never share a temp path.
3. Optional: take an advisory lock (e.g. `fs2`/`fd-lock`) across the
   read-modify-write window, or at least document the single-writer assumption.
   (Optional because the cross-process race is a low-severity edge case.)

### Acceptance Criteria

1. After fdemon edits an existing `chmod 600` rc file, the file's mode is still
   `0600` (Unix); a newly-created rc file is created `0600`, not `0644`.
2. The temp file used during the atomic write has a unique, non-deterministic name
   (no two invocations share `<rc_file>.fdemon_tmp`).
3. The atomic-rename behavior and idempotent fence-block handling are otherwise
   unchanged (existing rc-file tests stay green).

### Testing

```rust
// path_config.rs test module (Unix-gated for mode assertions)
// - #[cfg(unix)] test_write_rc_preserves_mode: create temp rc file, chmod 0600,
//     run add_to_rc_file, assert metadata mode is still 0600.
// - #[cfg(unix)] test_write_rc_new_file_is_0600: target a non-existent path, assert
//     created file is 0600.
// - temp-name uniqueness: assert the helper does not write a literal ".fdemon_tmp"
//     sibling (or that two helper calls choose distinct temp paths).
```

### Notes

- Shares `path_config.rs` with task 02 — run serially on the same branch
  (chain C: 02 → 07), not parallel worktrees.
- If `tempfile` is promoted from dev-dep to dep, note it in the task 12 docs update
  (DEVELOPMENT.md/ARCHITECTURE.md dependency mention if warranted).
