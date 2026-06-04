## Task: Polish prerequisites_guided_commands — Yum command + caveats + import (M2 + n2 + m6)

**Severity:** MAJOR (M2), NITPICK (n2), MINOR (m6)

**Objective**: Make the Linux `Yum` guided command runnable on the systems that
actually reach it, add a best-effort caveat for community-sourced package names,
and tidy the `PREREQ_KEY_GIT` reference — all within
`prerequisites_guided_commands` in `install_wizard/state.rs`.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs`: `detect_linux_package_manager`
  precedence (apt-get → dnf → yum → pacman → zypper), `PREREQ_KEY_*` constants.

### Details

**M2 — Yum command (MAJOR).** `state.rs:352-356` — the `LinuxPackageManager::Yum`
arm reuses the **dnf** command string verbatim (`sudo dnf install -y …`) under the
label "Install Linux prerequisites (yum)". But `detect_linux_package_manager`
(`prerequisites.rs:116-130`) reaches `Yum` **only when `dnf` is absent** (apt → dnf
→ yum precedence) — i.e. legacy yum-only RHEL7/CentOS7 — so the command fails with
`dnf: command not found` on exactly the platform that reaches it. Fix by emitting a
real yum command:

```
sudo yum install -y curl git unzip xz zip mesa-libGLU clang cmake ninja-build pkgconf gtk3-devel
```

(adjust package names to the yum/RHEL7 equivalents as appropriate). If you instead
keep treating yum as a dnf shim, that is **not acceptable on its own** — at minimum
the `note` must warn "On RHEL7/CentOS7 `dnf` may be absent; substitute `yum`."
Prefer the real `yum` command so the primary string works unmodified.

**n2 — best-effort caveat (NITPICK, optional).** `state.rs:347-366` — only the apt
package set is officially Flutter-documented; dnf/pacman/zypper names are
community-sourced. Each non-apt arm already carries an `or: <apt equivalent>` note.
Optionally add a short best-effort caveat (e.g. append to the non-apt notes, or a
one-line Prerequisites caption hint: "package names are best-effort; consult your
distro docs if a package is not found"). Implementer may defer this without blocking
the task.

**m6 — import consistency (MINOR).** `state.rs:426` references
`fdemon_daemon::toolchain::PREREQ_KEY_GIT` by full path, while the peer constants
`PREREQ_KEY_XCODE_CLT`/`PREREQ_KEY_COCOAPODS`/`PREREQ_KEY_ROSETTA` are imported in
the `use` block (~`9-13`) and used by short name. Add `PREREQ_KEY_GIT` to that `use`
block and use the short name at line 426. (If task 04 later moves winget detection
into the daemon, this stays valid — the missing-key check still uses the constant.)

### Acceptance Criteria

1. The `Yum` arm emits a command that runs on a yum-only system (real `yum install
   …`), or carries an explicit `dnf`→`yum` substitution note. No "(yum)"-labeled
   command silently invokes a missing `dnf`.
2. Existing per-`LinuxPackageManager` command-string tests are updated for the new
   Yum string; apt/dnf/pacman/zypper/Unknown arms are otherwise unchanged.
3. `PREREQ_KEY_GIT` is referenced by the short imported name, consistent with peers.
4. (Optional) a best-effort caveat note is present on the community-sourced arms.

### Testing

```rust
#[cfg(test)]
mod tests {
    // - test_prereq_guided_linux_yum_* now asserts the yum command string
    //   (real yum, or dnf+substitution-note), not a bare dnf command.
    // - other manager arms unchanged.
}
```

### Notes

- Keep all command strings as static literals selected by enum — do not interpolate
  dynamic input (the security review confirmed the current static-literal design is
  injection-safe; preserve it).
- This task only touches `state.rs`; it is parallel-safe with tasks 01 and 03.
