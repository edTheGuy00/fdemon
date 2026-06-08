## Task: Make `winget_available` symmetric with `linux_package_manager` (N3) — OPTIONAL, CROSS-CUTTING

**Severity:** NITPICK (optional — explicitly deferrable)

**Objective**: Resolve the type asymmetry on `ToolchainReport` where
`linux_package_manager: Option<LinuxPackageManager>` (None = "not probed / non-Linux")
sits beside `winget_available: bool` (where `false` conflates "probed, absent" with "not
probed / non-Windows"). Make the two fields symmetric.

**Depends on**: 02-scroll-window-selected-command, 03-reexport-linux-package-manager,
05-pm-caveat-symmetry (all share files this task re-touches — see overlap note)

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/types.rs` (field type)
- `crates/fdemon-daemon/src/toolchain/mod.rs` (`run_preflight` population)
- `crates/fdemon-app/src/install_wizard/state.rs` (consumer read + test fixtures)
- `crates/fdemon-app/src/handler/install_wizard/actions.rs` (test fixtures)
- `crates/fdemon-app/src/handler/install_wizard/navigation.rs` (test fixtures)
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` (test fixtures)
- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` (test fixtures)

**Files Read (Dependencies):**
- every `ToolchainReport` construction site (this is the cross-cutting surface).

### Details

**This is an optional NITPICK and is genuinely cross-cutting** — changing the
`winget_available` field type ripples to *every* `ToolchainReport` construction site
(the production `run_preflight` plus all app/tui test fixtures) and to the consumer in
`prerequisites_guided_commands`. The field is already documented (a comment notes the
`false` conflation) and is harmless (a single, platform-gated consumer). The risks
reviewer rated keeping it as-is "acceptable". **Defer this freely if the churn is not
worth it.**

If taken, choose ONE of:

**Option A (symmetry, preferred if done):** change the field to
`winget_available: Option<bool>` where `None` = "not probed (non-Windows)", `Some(true/
false)` = "probed". Update:
1. `run_preflight` (`toolchain/mod.rs`): populate `Some(which::which("winget").is_ok())`
   only on Windows; `None` elsewhere (mirrors how `linux_package_manager` is gated).
2. `prerequisites_guided_commands` (`state.rs`, Windows arm ~`:448`): read
   `report.winget_available == Some(true)` (or `.unwrap_or(false)`), preserving the exact
   same guided-command output as today.
3. Every `ToolchainReport { … }` test fixture across app + tui: set the new
   `Option<bool>` value (`None` for non-Windows fixtures, `Some(false)`/`Some(true)` where
   the test exercises the Windows arm).

**Option B (document-and-close):** if deferring the type change, instead strengthen the
field doc-comment on `types.rs` to explicitly state the `false`-conflation contract and
note that consumers must gate on `report.platform` before trusting it — then mark N3
resolved-by-documentation. (This is the lower-churn path.)

Either way, **guided-command output must be byte-for-byte unchanged** for every platform.

### Acceptance Criteria

1. Either `winget_available` is `Option<bool>` (gated like `linux_package_manager`) with
   all construction sites updated, **or** its `bool` contract is explicitly documented and
   N3 is closed as documented-by-design.
2. Guided-command output is unchanged for every platform/manager.
3. `cargo check --workspace --all-targets`, `cargo test --workspace`, and
   `cargo clippy --workspace --all-targets -- -D warnings` are green.

### Testing

```rust
#[cfg(test)]
mod tests {
    // - if Option<bool>: run_preflight yields None on non-Windows, Some(_) on Windows
    //   (or assert via a constructed report where mocking PATH is impractical).
    // - Windows guided-command output identical to before for winget present/absent.
}
```

### Notes

- **Cross-cutting and optional.** It overlaps tasks 02/03 (`step_detail.rs` fixtures + tui
  `mod.rs`) and task 05 (`state.rs`), so it must run **last**, after those land — hence the
  dependency edges. It does not overlap task 07 (`ARCHITECTURE.md`), so the two may run in
  parallel in the final wave.
- If deferred, say so explicitly in the followup status — do not silently drop it.
