## Task: Refine prerequisites.rs detection — prefix/status consistency + Windows soften + GTK (m2 + m3 + m4 + n1)

**Severity:** MINOR (m2, m3, m4), NITPICK (n1)

**Objective**: Bring the Linux detection branch into line with the documented
`MISSING_PREFIX` / `ComponentStatus` contract, stop the Windows check from
overstating readiness, and avoid asserting GTK absence when the probe is
undeterminable — all read-only detection refinements in `prerequisites.rs`.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs`: `ComponentStatus` doc (`Partial` =
  "present but degraded", `Missing` = "not found").

### Details

**m2 — Linux detail prefix (MINOR).** `prerequisites.rs:219` (`check_linux_prerequisites`)
and `:490` (`build_linux_check_from_missing` test helper) emit `format!("Missing: {}", …)`
with **capital-M**, while `MISSING_PREFIX` (`:49`) is lowercase `"missing: "` and
macOS/Windows use `format!("{}{}", MISSING_PREFIX, …)` (`:391`, `:458`). Today Linux
does not route through `parse_missing_prereq_keys`, so this is latent — but it is an
ad-hoc string mimicking the documented contract. Either (a) route Linux through
`MISSING_PREFIX` too, **or** (b) add an explicit comment at both sites stating Linux
intentionally does not embed parseable keys (because the guided-command path uses
`detect_linux_package_manager`, not key parsing). Prefer (a) for uniformity unless it
forces a semantic change you do not want.

**m3 — Partial vs Missing (MINOR).** `check_linux_prerequisites` (`:216-221`) maps
**any** missing item — including hard-required binaries (git, cmake, ninja, …) — to
`ComponentStatus::Partial`, whereas macOS/Windows map equivalent definitive absences
to `ComponentStatus::Missing` (`build_macos_check_from_statuses :390`,
`build_windows_check_from_presence :457`). Per `types.rs` doc, `Partial` means
"present but degraded", so labeling a wholly-absent required tool `Partial`
contradicts the contract and rolls up differently (`rollup_status` in app
`state.rs` → `StepStatus::Partial` on Linux vs `Missing` on Windows). Decide and
**document** the intended semantics: either return `Missing` when a required binary
is absent (reserving `Partial` for the GTK-headers-only degraded case), or add a
one-line comment explaining the deliberate divergence. Functionally harmless today
(both are non-Ok), so this is a consistency/contract fix, not a behavior change
unless you choose option A.

**m4 — Windows false-Ok (MINOR).** `check_windows_prerequisites` /
`build_windows_check_from_presence` (`:414-461`) gate `ComponentStatus` solely on
git; VS "Desktop development with C++" detection is deferred (note-only). A Windows
user with git but no MSVC C++ toolchain sees `Ok` and hits an opaque Flutter build
failure later. Interim fix (no new probe): soften the Windows `Ok` detail text to
flag the unverified workload, e.g. append `"Visual Studio C++ build tools not
verified; install \"Desktop development with C++\" if Windows desktop builds fail"`.
(A real `vswhere.exe` probe remains out of scope — explicitly deferred in Phase 4.)

**n1 — GTK double-report (NITPICK, optional).** `prerequisites.rs:203-206` — when
`pkg-config`/`pkgconf` is absent, the binary loop already pushes `pkg-config` to the
missing list; `probe_pkg_config_exists` then fails to spawn, returns `false`, and
`libgtk-3-dev` is **also** pushed even though GTK presence is genuinely
undeterminable without pkg-config. Optionally compute a `pkg_config_found` flag in
the binary loop and only push GTK as missing when `pkg_config_found && !gtk_present`
(or label it "undetermined"). Harmless today (install command covers both); defer-able.

### Acceptance Criteria

1. Linux missing-item detail either uses `MISSING_PREFIX` or carries an explicit
   comment documenting the intentional opt-out; no silent capital-M ad-hoc string.
2. Linux `ComponentStatus` for absent **required** tools is either `Missing`
   (matching macOS/Windows) or its `Partial` use is documented at the source as
   deliberate.
3. The Windows `Ok` detail text no longer overstates readiness — it flags the
   unverified VS C++ workload.
4. (Optional) GTK is not reported as definitively missing when `pkg-config` itself
   is absent.
5. Detection stays strictly read-only (no installs, no shell, `Stdio::null`).

### Testing

```rust
#[cfg(test)]
mod tests {
    // - linux detail/status: assert the chosen contract (prefix or documented opt-out;
    //   Missing-or-documented-Partial for absent required tool).
    // - windows Ok detail contains the VS C++ caveat substring.
    // - (optional n1) pkg-config-absent case does not assert libgtk-3-dev missing,
    //   or labels it undetermined.
    // - existing macOS/Windows status-mapping + parse round-trip tests stay green.
}
```

### Notes

- Update `build_linux_check_from_missing` (test helper) and the affected
  `test_missing_*` assertions to match whichever contract you choose for m2.
- This task only touches `prerequisites.rs`; parallel-safe with tasks 01 and 02.
- Task 04 will later move package-manager/winget detection into the daemon report;
  keep this task's changes confined to the existing check functions so 04 can build
  on top.
