## Task: Fix prerequisite probes — pkgconf alias, Rosetta idle daemon, dead xz-utils alias (F-PR53-10 + dead alias)

**Severity:** MEDIUM (correctness — misleading guidance)

**Objective**: Eliminate three prerequisite-detection defects that produce
incorrect "missing" guidance: the `pkgconf` alias path probes the wrong binary,
Rosetta is inferred from a daemon that may merely be idle, and a dead `xz-utils`
alias can never match.

**Depends on**: — (disjoint; safe to parallelize)

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/checks/mod.rs` (ComponentStatus aggregation / missing_keys contract)

### Details

**(a) pkgconf alias probes the wrong binary.**
`prerequisites.rs:220-230`: when `pkg-config` is absent but the `pkgconf` alias is
present, `pkg_config_found = true`. But the GTK/GLU probes
(`probe_pkg_config_exists`, 432-446) hardcode `Command::new("pkg-config")`. The
alias branch is reached only when `which::which("pkg-config")` already failed, so
the probe always spawns a non-existent binary → `gtk_missing = true` and
`glu_missing = true` even when the dev-headers are installed (Fedora/Arch minimal
installs that ship `pkgconf` without a `pkg-config` symlink). Unit tests miss it
because `build_linux_check_from_candidates` takes `gtk_present`/`glu_present` as
parameters, bypassing the real probe.

**(b) Rosetta idle-daemon false negative.**
`probe_macos_rosetta` (`prerequisites.rs:515-539`, aarch64-gated) infers Rosetta
from `pgrep oahd`: a non-zero exit (no live `oahd`) maps to
`MacOsProbeStatus::Missing`. `oahd` is the on-demand Rosetta daemon — not running
after a fresh boot or before any Intel binary launches — so installed Rosetta is
reported Missing, forcing the whole Prerequisites component to Missing with
`rosetta` in missing_keys. (Note: line 536 already maps "pgrep not found" → Unknown,
so the author distinguishes Unknown from Missing elsewhere.)

**(c) Dead `xz-utils` alias.**
`prerequisites.rs:216-218` (and the install-instruction path at 349) probe
`which::which("xz-utils")`. `xz-utils` is a Debian package name; the binary it
installs is `xz`. No platform installs an `xz-utils` executable, so the alias arm
is dead and its comment describes a nonexistent situation.

### Proposed Fix

1. Make `probe_pkg_config_exists` take the resolved binary name (or record which of
   `pkg-config`/`pkgconf` was found) and spawn that binary for the GTK/GLU
   `--exists` probes.
2. Detect Rosetta by **installation**, not a running process: check a runtime path
   (e.g. `/Library/Apple/usr/share/rosetta/rosetta` or
   `/Library/Apple/usr/libexec/oah`) or `pkgutil --pkg-info=com.apple.pkg.RosettaUpdateAuto`.
   At minimum, treat a non-running `oahd` as `Unknown` rather than `Missing`.
3. Remove the dead `xz` → `xz-utils` alias arm (both the `which` check and the
   install-instruction reference) and its misleading comment.

### Acceptance Criteria

1. With `pkg-config` absent but `pkgconf` present, the GTK/GLU probes run against
   `pkgconf` and correctly report present headers (no false "missing").
2. On Apple Silicon with Rosetta installed but `oahd` idle, Prerequisites is not
   reported Missing on account of `rosetta` (Present via installation probe, or at
   worst Unknown — never a false Missing).
3. The `xz-utils` alias arm and comment are gone; `which::which("xz")` remains the
   sole `xz` check.

### Testing

```rust
// prerequisites.rs test module
// - Refactor so the binary-name selection (pkg-config vs pkgconf) is testable
//   without spawning; assert the resolved name is passed to the probe.
// - Rosetta: factor the install-detection into a pure fn over injected path-exists /
//   pkgutil results and unit-test the installed-but-idle case -> Present/Unknown.
// - assert the LINUX_REQUIRED_TOOLS alias table no longer contains "xz-utils".
```

### Notes

- File-disjoint from all other tasks → Wave 1 parallel worktree candidate.
- All three are "misleading guidance" defects, not crashes; the value is correct
  diagnostics for real-world minimal Linux installs and Apple Silicon machines.
