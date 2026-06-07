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

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/Cargo.toml` | Promoted `tempfile` from `[dev-dependencies]` to `[dependencies]` |
| `crates/fdemon-daemon/src/toolchain/path_config.rs` | Added `#[cfg(unix)] use std::os::unix::fs::PermissionsExt`; rewrote `write_rc_atomically` to use `tempfile::Builder::new().prefix(".fdemon-rc-tmp-").tempfile_in(parent)` for unique temp name and `persist()` for atomic rename; added Unix permission preservation (reads existing mode, applies to temp file before rename; defaults to 0600 for new files); added 3 new tests: `test_write_rc_preserves_mode` (#[cfg(unix)]), `test_write_rc_new_file_is_0600` (#[cfg(unix)]), `test_write_rc_temp_name_is_not_deterministic_fdemon_tmp` |

### Notable Decisions/Tradeoffs

1. **Using `tempfile::Builder::new().tempfile_in(parent).persist()`**: This is the idiomatic `tempfile` API for unique atomic writes. `persist()` consumes the `NamedTempFile` and renames it to the destination, preventing the Drop impl from deleting it. Error on `persist` returns a `PersistError` with both the error and the original file accessible.
2. **Permission preservation via snapshot**: Reading metadata before writing and applying after, rather than using `O_CREAT` flags, is the only portable approach on stable Rust without unsafe code. The read-then-apply window is minimal (same process, same thread).
3. **Best-effort permission application**: If `set_permissions` fails (e.g., no ownership), a debug trace is logged but the write proceeds. The rename still completes — the content is correct even if the mode restore failed.
4. **`parent` fallback to `.`**: When `rc_file.parent()` returns `None` or an empty path (e.g., bare filename), we fall back to `"."`. This is consistent with the existing `create_dir_all` logic.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-daemon --lib -- toolchain::path_config` - Passed (85 tests, 3 new)
- `cargo test --workspace` - Passed (all test results ok, no failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Cross-process race still possible**: The task notes this is optional. No advisory lock is taken across the read-modify-write window. Two concurrent fdemon processes can still produce a lost-update if they both read at the same time. The unique temp name eliminates the temp-clobber hazard but not the logical race. Documented as single-writer assumption in the doc comment.
2. **Non-Unix platforms**: Permission handling is a no-op on non-Unix platforms (Windows uses registry, not rc files). This is correct behavior — `write_rc_atomically` is only called for Unix rc files.

### Doc Updates Needed

- `docs/ARCHITECTURE.md` `fdemon-daemon` dependency list: `tempfile` promoted from dev-dep to regular dep (used in non-test code for unique temp file creation in `toolchain/path_config.rs`). Task 12 should note this.
