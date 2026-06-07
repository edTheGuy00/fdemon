## Task: Fix cross-platform correctness in Android/JDK install (F-PR53-09 + relocate atomicity)

**Severity:** MEDIUM (correctness)

**Objective**: Fix three platform/heuristic defects in the Android & JDK install
path: a hardcoded POSIX `:` PATH separator that corrupts PATH on Windows, a
`java_home_from_which` heuristic that returns `/usr` for a non-JDK `java` stub,
and a non-atomic `relocate_cmdline_tools` that can destroy an existing install.

**Depends on**: 03 (shares `android_install.rs`)

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/android_install.rs`
- `crates/fdemon-daemon/src/toolchain/jdk.rs`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/checks/android.rs` (`sdkmanager_bin_name` returns `sdkmanager.bat` on Windows)

### Details

**(a) POSIX `:` PATH separator on Windows.**
`android_install.rs:322-326` builds the child PATH for sdkmanager via
`format!("{jdk_bin}:{existing_path}")` — a hardcoded `:`. On Windows the
separator is `;`, so the entire inherited PATH collapses into one mangled entry.
Reachable on Windows (`sdkmanager.bat`), gated on `target.jdk_path.is_some()`.
(`JAVA_HOME` is also set at line 313, which mitigates the java lookup, but other
PATH-resolved tools the child needs can break.)

**(b) Bogus `/usr` JAVA_HOME.**
`jdk.rs:125-141` (`java_home_from_which`) takes the grandparent of the resolved
`java` and accepts it if `<home>/release` is a file **OR** `<home>/lib` is a dir.
For a stub `java` at `/usr/bin/java` (notably macOS, where it is a real binary
not a symlink into a JDK), `canonicalize` leaves it at `/usr/bin/java`, the
grandparent is `/usr`, `/usr/lib` exists everywhere → returns `/usr`, which flows
into `flutter config --jdk-dir=/usr`. The doc at line 120 says `/usr/bin/java`
should be skipped, but the code does not. (Typical Linux is mitigated by
`canonicalize` following the alternatives symlink chain into the real JDK.)

**(c) Non-atomic `relocate_cmdline_tools`.**
`android_install.rs:450-468` claims "atomic relocation" but does
`remove_dir_all(existing latest/)` **then** `rename`. A failed rename after the
remove leaves the user with no `cmdline-tools/latest` — a previously working
install destroyed, with no backup.

### Proposed Fix

1. Build the child PATH with `std::env::join_paths` over
   `std::env::split_paths(&existing)` prepended with the JDK bin dir (or branch on
   `cfg!(windows)` for the separator). Prefer `join_paths` for correct quoting.
2. Tighten `java_home_from_which`: require the canonical `release` file **or** a
   JDK marker like `<home>/bin/javac` (and/or `<home>/lib/libjvm.*`); reject
   well-known non-JDK prefixes (`/usr`, `/usr/local`). Honor the documented
   `/usr/bin/java` skip.
3. Make `relocate_cmdline_tools` genuinely atomic: rename an existing `latest/` to
   a backup (`latest.bak-<pid>`), perform the new rename, delete the backup on
   success, and **restore** it on failure. Alternatively, if a backup-restore is
   deemed overkill, soften the "atomic" docstring to reflect destroy-then-replace —
   but backup-restore is preferred since it protects a working install.

### Acceptance Criteria

1. The sdkmanager child PATH is assembled with the OS-correct separator (verified
   via `join_paths`/`split_paths`); on Windows the inherited PATH entries remain
   individually resolvable.
2. `java_home_from_which` returns `None` for a `java` resolving to `/usr/bin/java`
   that is not a JDK home, and still returns the correct home for a real JDK layout
   (with `release`/`bin/javac`).
3. A simulated rename failure in `relocate_cmdline_tools` leaves the pre-existing
   `cmdline-tools/latest` intact (restored from backup) — no destroyed install.

### Testing

```rust
// jdk.rs test module
// - test_java_home_rejects_usr_stub: fixture where java -> /usr/bin/java with no
//   release/javac under /usr -> returns None.
// - test_java_home_accepts_real_jdk: fixture with <home>/release and bin/javac -> Some(home).
// android_install.rs test module
// - PATH separator: factor the child-PATH build into a pure fn taking the separator
//   (or split/join) and unit-test the Windows (';') and Unix (':') results.
// - relocate backup-restore: create an existing latest/, force the inner rename to
//   fail, assert latest/ is restored.
```

### Notes

- Depends on task 03 (both write `android_install.rs`); runs after 03. File-disjoint
  from task 04, so 04 and 05 may run in parallel worktrees after 03.
- Most impact is platform-specific (Windows / macOS); ensure the pure helpers are
  testable on the CI host (Linux) and gate any OS-specific integration behind cfg.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/android_install.rs` | (a) PATH separator fixed via `join_paths`/`split_paths`; (c) `relocate_cmdline_tools` backup-restore pattern; new tests for PATH separator and backup-restore |
| `crates/fdemon-daemon/src/toolchain/jdk.rs` | (b) `java_home_from_which` tightened: added `NON_JDK_PREFIXES` constant, reject `/usr`/`/usr/local`, require `release` file or `bin/javac` (not bare `lib/`); new unit tests |

### Notable Decisions/Tradeoffs

1. **PATH separator (a)**: Used `std::env::split_paths` + `std::env::join_paths` which handles both `:` (POSIX) and `;` (Windows) automatically and correctly quotes paths with spaces. No `cfg!(windows)` branching needed.

2. **JDK prefix rejection (b)**: Added a `NON_JDK_PREFIXES: &[&str]` constant (`/usr`, `/usr/local`) that is checked before the JDK marker test. The marker check was tightened from `release file OR lib/` to `release file OR bin/javac` — the old `lib/` check was the root cause of the `/usr` false positive since `/usr/lib` exists everywhere. All new tests are pure (no `which` invocation needed) and run on Linux CI.

3. **Backup-restore (c)**: The `relocate_cmdline_tools` function now renames an existing `latest/` to `latest.bak-<pid>` before attempting the new rename. On success, the backup is removed (best-effort, with a warning log if removal fails). On rename failure, the backup is restored via `rename`, preserving the pre-existing working install. On catastrophic restore failure (both renames fail), an `error!` log is emitted with enough context for manual recovery.

### Testing Performed

- `cargo fmt --all -- --check` - PASS
- `cargo check --workspace --all-targets` - PASS
- `cargo test -p fdemon-daemon --lib -- toolchain::android_install::tests` - PASS (20 tests)
- `cargo test -p fdemon-daemon --lib -- toolchain::jdk::tests` - PASS (13 tests)
- `cargo test --workspace` - PASS (all test suites green)
- `cargo clippy --workspace --all-targets -- -D warnings` - PASS

### Risks/Limitations

1. **`join_paths` fallback**: If `join_paths` fails (unusual characters in PATH entries), the code falls back to the original existing PATH unchanged. The fallback is logged implicitly via the `unwrap_or_else` — adding explicit tracing would be marginal since this case is exceedingly rare in practice.

2. **Non-JDK prefix list is POSIX-only**: `NON_JDK_PREFIXES` only covers `/usr` and `/usr/local`. On Windows the equivalent would be `C:\Windows\System32`, but the `which` heuristic is less likely to fire incorrectly there since Windows JDK installs always have `bin\javac.exe`. No Windows-specific prefix was added since it would require `#[cfg(windows)]` gating and there are no Windows CI hosts to verify against.
