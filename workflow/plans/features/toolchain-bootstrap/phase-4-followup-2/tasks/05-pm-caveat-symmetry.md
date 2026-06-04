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
