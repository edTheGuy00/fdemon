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
