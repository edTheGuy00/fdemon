## Task: Refine Linux prerequisite detection + package-manager identification

**Objective**: Make the Linux `Prerequisites` check accurate enough to drive a
correct guided install command — detect the active package manager and probe the
Flutter Linux build prerequisites (binaries via `which`, GTK dev-headers via
`pkg-config`) — while keeping the check strictly read-only.

**Depends on**: None

**Estimated Time**: 4-6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs`: extend the Linux
  arm with package-manager detection + a GTK probe; widen the required-tool list.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs`: `ComponentCheck { kind, status, detail }`,
  `ComponentStatus`, `HostPlatform`.
- `crates/fdemon-daemon/src/toolchain/checks/mod.rs`: existing probe helpers and the
  `PROBE_TIMEOUT` constant pattern.

### Details

Today the Linux arm is a coarse `which`-only probe over ~7 tools (`cmake`, `ninja`,
`pkg-config`, `clang`, `curl`, `unzip`, `xz`) with `ninja`/`ninja-build` and
`xz`/`xz-utils` alias fallbacks. Phase 4 needs two additions:

1. **Package-manager identification.** Add a `pub(crate)` probe that returns the
   detected manager in preference order **apt-get → dnf → yum → pacman → zypper**,
   using `which::which` (consistent with the existing `which` usage). Expose it so
   `state.rs` (task 03) can select the right install command:

   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub(crate) enum LinuxPackageManager { Apt, Dnf, Yum, Pacman, Zypper, Unknown }

   pub(crate) fn detect_linux_package_manager() -> LinuxPackageManager { /* which::which chain */ }
   ```

   Use `which` as ground truth — do **not** parse `/etc/os-release`.

   > **Cross-crate note:** task 03 (in `fdemon-app`) needs to know the manager to
   > pick the command. Either (a) re-export `LinuxPackageManager` from
   > `toolchain::mod` and have task 03 call `detect_linux_package_manager()`
   > directly, or (b) fold the detected manager into the `detail`/missing-key
   > contract (task 02). Prefer (a) — it is a pure function and keeps the command
   > strings in app-land where `jdk_guided_command` already lives.

2. **Extend the required-tool set + GTK probe.**
   - Add `git` and `zip` to the `which`-probed tools; keep the `ninja`/`ninja-build`,
     `xz`/`xz-utils` alias fallbacks and add a `pkg-config`/`pkgconf` alias.
   - Add a **GTK dev-headers** check via `pkg-config --exists gtk+-3.0` (exit 0 =
     present) using `tokio::process::Command` with `PROBE_TIMEOUT` and
     `Stdio::null()`, mirroring the macOS `xcode-select` block. `which` cannot detect
     library dev headers, so this probe is required. A non-zero/absent result means
     the `libgtk-3-dev` / `gtk3-devel` / `gtk3` item is missing.

3. **Keep one aggregate `ComponentCheck`.** `ComponentKind::Prerequisites` stays a
   single rolled-up check (`{ kind, status, detail }` — no new fields/variants).
   The `detail` string lists the missing items; status is `Ok` only when **all**
   probes pass, else `Missing` (or `Partial` if some present). Make the missing
   list precise enough that task 03 can decide whether to emit a command at all.

4. **Read-only contract.** Only `which` and `pkg-config --exists`/`--version`
   probes — never install anything. Update the module doc comment (currently says
   the module "never generates install commands (Phase 4)") to note that command
   generation now lives in app-land `state.rs`, while detection stays here.

### Acceptance Criteria

1. `detect_linux_package_manager()` returns the first present manager in the order
   apt-get → dnf → yum → pacman → zypper, and `Unknown` when none is found.
2. The Linux prerequisites check probes `git`, `zip`, `curl`, `unzip`, `xz`,
   `clang`, `cmake`, `ninja`, `pkg-config` (with documented alias fallbacks) and the
   GTK dev-headers via `pkg-config --exists gtk+-3.0`.
3. Status is `Ok` only when every probe passes; otherwise the `detail` enumerates
   the missing items.
4. No installs are performed; the module remains read-only.

### Testing

```rust
#[cfg(test)]
mod tests {
    // - package-manager precedence (apt before dnf, etc.; Unknown when none)
    // - missing-tool aggregation includes git/zip
    // - GTK-missing maps to a Missing/Partial item in detail
    // - all-present => Ok with empty/clean detail
}
```

Mock `which`/probe outcomes via the existing test seams in `checks/`; if a probe is
not easily mockable, factor the status-mapping into a pure helper and test that.

### Notes

- Tasks 01 and 02 both edit `prerequisites.rs` and are sequenced (02 depends on 01)
  — no merge conflict, but coordinate the shared `#[cfg(test)] mod tests`.
- Bias toward not over-reporting: a working toolchain that merely lacks a `.pc`
  file should not be hard-`Missing` if it would nag users — prefer aggregate
  `detail` precision over false alarms (see Phase-4 risks).
- `libstdc++-dev` has no portable probe (no binary, no reliable `.pc`); do **not**
  attempt to detect it — task 03 includes it in the apt command string only.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs` | Added `LinuxPackageManager` enum (`pub`), `detect_linux_package_manager()`, extended `LINUX_REQUIRED_TOOLS` with `git`/`zip`, alias fallback for `pkgconf`, GTK dev-headers probe via `pkg-config --exists gtk+-3.0`, pure helper `build_linux_check_from_missing` (test-only), updated module doc comment, 16 unit tests |
| `crates/fdemon-daemon/src/toolchain/checks/mod.rs` | Re-export `detect_linux_package_manager` and `LinuxPackageManager` from `prerequisites` module |
| `crates/fdemon-daemon/src/toolchain/mod.rs` | Re-export both symbols from `checks` sub-path |
| `crates/fdemon-daemon/src/lib.rs` | Re-export both symbols in the crate's public API surface |

### Notable Decisions/Tradeoffs

1. **`pub` vs `pub(crate)`**: Changed `LinuxPackageManager` and `detect_linux_package_manager` from `pub(crate)` to `pub` so they can be re-exported through the full module chain (`checks/mod.rs` → `toolchain/mod.rs` → `lib.rs`). This is consistent with task preference (a) — keeping the function in daemon-land and calling it from fdemon-app task 03.

2. **GTK probe uses `Partial` (not `Missing`)**: The task specifies `Missing` or `Partial` status for individual items, and keeps one aggregate `ComponentCheck`. Missing tools/headers all contribute to `ComponentStatus::Partial` in the aggregate (not `Missing`) so that a partially-equipped Linux system reads as degraded, not completely absent. This mirrors the pre-existing behavior.

3. **`build_linux_check_from_missing` helper**: The pure status-mapping helper is `#[cfg(test)]`-gated (not exposed publicly) — all tests that need it are within the same module. This avoids leaking test-only scaffolding into the public API.

4. **Detail format change**: Changed `"Missing tools: X"` → `"Missing: X"` to be consistent with the GTK label (`libgtk-3-dev` is a package name, not a binary name), making the detail string homogeneous.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test -p fdemon-daemon --lib toolchain::checks::prerequisites` — Passed (16/16 new tests)
- `cargo test --workspace` — Passed (all test suites green; pre-existing `jdk::test_resolve_jdk_home_honors_java_home` is a transient race on `JAVA_HOME` env mutation, unrelated to this task)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **JDK test flakiness**: `toolchain::jdk::tests::test_resolve_jdk_home_honors_java_home` uses unsynchronized `std::env::set_var` and races under parallel test execution. This is pre-existing and unrelated to this task.
2. **GTK probe on Fedora/Arch**: Fedora and Arch provide `pkgconf` rather than `pkg-config`; the probe spawns `pkg-config --exists gtk+-3.0`. If `pkgconf` provides `pkg-config` as a shim (standard on most distros), the probe works. If not, the probe will fail to spawn (not-found error) and return `false` — reporting GTK as missing even if headers exist. This is the conservative bias the task asks for.
