## Task: Make the best-effort caveat symmetric across non-apt PM arms (N2) — OPTIONAL

**Severity:** NITPICK (optional)

**Objective**: The first-round followup added a "best-effort package names" caveat to the
`Yum` arm only. The `Dnf`, `Pacman`, and `Zypper` arms carry the same community-sourced
wrong-package risk but no equivalent caveat. Make caveat coverage consistent.

**Depends on**: None

**Estimated Time**: 0.5-1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain` — `LinuxPackageManager` arms.

### Details

`prerequisites_guided_commands` in `state.rs` (the Linux arms, ~`:351-380`) builds one
`GuidedCommand` per detected package manager. Only the apt package set is
Flutter-documented; dnf/yum/pacman/zypper names are community-sourced. Each non-apt arm
already carries an `or: <apt equivalent>` note, and the `Yum` arm additionally carries a
best-effort caveat (added in the first-round followup for n2). The `Dnf`, `Pacman`, and
`Zypper` arms do not.

**Fix:** Add a short, consistent best-effort caveat to the `Dnf`, `Pacman`, and `Zypper`
arms' notes (mirroring the `Yum` arm's wording), e.g. append/include:
*"Package names are best-effort; consult your distro docs if a package is not found."*

Keep all command strings and notes as **static literals selected by enum** — do not
interpolate any dynamic input (the security review confirmed and requires the
static-literal design). Do not change the command strings themselves; this only adds/edits
the `note` text on the non-apt arms.

### Acceptance Criteria

1. The `Dnf`, `Pacman`, and `Zypper` arms each carry a best-effort caveat consistent with
   the `Yum` arm; the `Apt` arm (officially documented) does not need one.
2. Command strings are unchanged; only `note` text is added/adjusted.
3. All strings remain static literals (no dynamic interpolation).
4. Existing per-`LinuxPackageManager` tests updated for the new note text where they assert
   note contents; command-string assertions unchanged.

### Testing

```rust
#[cfg(test)]
mod tests {
    // - assert each non-apt arm's note contains the best-effort caveat substring.
    // - command-string assertions for all arms remain unchanged.
}
```

### Notes

- Optional NITPICK — may be deferred without blocking. Touches only `state.rs`;
  parallel-safe with tasks 01 and 04.
- If you instead prefer a single Prerequisites caption hint over per-arm notes, that is
  acceptable as long as the best-effort message reaches the user for every non-apt manager.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a258de703313fb533

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/install_wizard/state.rs` | Updated `Dnf`, `Pacman`, and `Zypper` arm notes in `prerequisites_guided_commands` to include the best-effort caveat; updated tests to assert `note.contains("best-effort")` for those three arms |

### Notable Decisions/Tradeoffs

1. **Combined caveat + "or:" note**: Rather than replacing the existing `"or: sudo apt-get..."` alternative note, the best-effort caveat was prepended to it for `Dnf`, `Pacman`, and `Zypper` arms. This preserves the useful fallback alternative while adding the required caveat. The `Yum` arm keeps its standalone caveat (yum-only systems don't have `dnf` so the `apt` fallback isn't useful there).
2. **Consistent caveat wording**: Used `"Package names are best-effort; consult your distro docs if a package is not found."` matching the spirit of the `Yum` arm wording, generalised without the RHEL7/CentOS7 specificity since that's only relevant for yum.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-app --lib -- install_wizard::state::tests` - Passed (74 tests)
- `cargo test --workspace` - Passed (all tests; pre-existing flaky env-var test unrelated to our changes)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Pre-existing flaky test**: `toolchain::flutter_install::tests::test_resolve_install_dir_fvm_cache_path_env` can fail when run in parallel with other tests due to environment variable pollution — not caused by this change.
