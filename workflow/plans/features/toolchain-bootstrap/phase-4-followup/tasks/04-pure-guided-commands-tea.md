## Task: Make prerequisites_guided_commands pure — move PATH detection into the daemon report (m1 + n4 + n3)

**Severity:** MINOR (m1), NITPICK (n4, n3)

**Objective**: Remove the synchronous `which::which` filesystem I/O from the TEA
`update()` path by pre-computing package-manager and winget availability in the
async daemon preflight and carrying the result on `ToolchainReport`, so
`prerequisites_guided_commands` becomes a pure function of the report. This also
removes the `which` dependency from `fdemon-app` (n4).

**Depends on**: 02-polish-prerequisites-guided-commands, 03-refine-prereq-detection-status
(both re-touch `state.rs` / `prerequisites.rs` — sequence after them)

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/types.rs`: add detected-environment fields to
  `ToolchainReport`.
- `crates/fdemon-daemon/src/toolchain/mod.rs`: populate the new fields in `run_preflight`.
- `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs`: expose the detection
  helpers used by `run_preflight` (already `pub`: `detect_linux_package_manager`; add
  a winget-availability probe if not already present).
- `crates/fdemon-app/src/install_wizard/state.rs`: `prerequisites_guided_commands`
  reads the pre-computed values from the report instead of calling `which::which`.
- `crates/fdemon-app/Cargo.toml`: drop the `which` dependency (added in Phase 4).

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/checks/mod.rs`, `crates/fdemon-daemon/src/lib.rs`:
  re-export chain.

### Details

**m1 — TEA purity (MINOR).** `prerequisites_guided_commands` (app `state.rs:316`,
Linux arm `:340`, Windows arm `:432`) calls `detect_linux_package_manager()` (up to
5 `which::which` PATH probes) and `which::which("winget")` (1 probe) **synchronously**.
The call chain is `Message::ToolchainPreflightCompleted` → `handle_preflight_completed`
→ `apply_report` → `build_steps` → `prerequisites_guided_commands`, i.e. inside the
`update()` dispatcher, which is contractually pure (`docs/REVIEW_FOCUS.md`; the only
approved exceptions are `Cell` render-hints and `version_check` network I/O). The
detection itself is fine — it just belongs in the async preflight task, not `update()`.

**Recommended fix (pure):**
1. Add fields to `ToolchainReport` (e.g. `pub linux_package_manager:
   Option<LinuxPackageManager>` and `pub winget_available: bool`, gated/populated
   per `platform`).
2. Populate them in `run_preflight` (async, in `fdemon-daemon`), where all other
   detection I/O already lives.
3. Change `prerequisites_guided_commands(platform, components)` →
   `prerequisites_guided_commands(report)` (or add the two values as params) and read
   the pre-computed values. No `which::which` in app-land.
4. Remove `which.workspace = true` from `crates/fdemon-app/Cargo.toml` (n4) — confirm
   no other app-land use remains (`grep which::`), then drop it.

**Acceptable lightweight alternative (if the refactor is deemed too invasive for a
MINOR):** keep the calls but annotate both sites with the standard exception comment:

```rust
// EXCEPTION (TEA): read-only PATH probe at preflight-completion time, not render.
// See docs/REVIEW_FOCUS.md approved-exceptions policy.
```

and add `which`-probe-in-update to the REVIEW_FOCUS.md approved list (that doc is
implementor-editable). The pure refactor is preferred because it also resolves n4
(drops the dependency) and keeps `build_steps` a pure function of the report.

**n3 — typed missing-keys (NITPICK, deferred note only).** While `ToolchainReport`
is being extended, the reviewer noted the stringly-typed `detail` →
`parse_missing_prereq_keys` cross-crate contract could eventually be replaced by a
typed `Vec<&'static str>` / enum-set field on `ComponentCheck`, eliminating the parse
path. **Do not implement this here** — it is future hardening, not a blocker. Add a
one-line `// TODO(phase-4-followup n3):` note near the detail/parse contract pointing
at this finding so it is discoverable. Track separately if pursued.

### Acceptance Criteria

1. `prerequisites_guided_commands` performs **no** `which::which` (or other
   filesystem/process) I/O; it derives commands purely from the report (and
   `HostPlatform`).
2. Package-manager and winget availability are detected in the async `run_preflight`
   and carried on `ToolchainReport`.
3. `which` is removed from `crates/fdemon-app/Cargo.toml` (and `Cargo.lock`), with no
   remaining `which::` usage in `fdemon-app` — **or**, if the lightweight alternative
   is chosen, both call sites carry the `// EXCEPTION:` annotation and the policy doc
   is updated.
4. Guided-command output is byte-for-byte unchanged for every platform/manager
   relative to before this task (pure relocation of *where* detection runs).
5. A `// TODO(phase-4-followup n3)` note marks the typed-missing-keys hardening idea.

### Testing

```rust
#[cfg(test)]
mod tests {
    // daemon: run_preflight populates linux_package_manager / winget_available
    //   appropriately for the host (or assert the field types/defaults via a
    //   constructed report where mocking PATH is impractical).
    // app: prerequisites_guided_commands(report) produces the same per-OS commands
    //   as the prior signature given an equivalent report — update existing tests to
    //   the new signature; no behavior change.
}
```

### Notes

- This is a refactor of *where* detection runs, not *what* it detects — preserve the
  exact command strings (including task 02's Yum fix) and the empty-when-all-Ok
  behavior.
- Sequenced after 02 and 03 because all three re-touch `state.rs` / `prerequisites.rs`;
  running 04 last on those files avoids merge churn.
- Keeps the daemon as the single home for environment detection I/O, consistent with
  `docs/ARCHITECTURE.md` layer assignments.
