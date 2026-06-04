# Action Items: Phase 4 Prerequisites — Followup Remediation

**Review Date:** 2026-06-04
**Verdict:** ⚠️ NEEDS WORK (no blocking/CRITICAL issues)
**Blocking Issues:** 0
**Confirmed findings:** 3 MINOR, 3 NITPICK, 1 deferred/tracked

The original phase-4-prereq review's findings are all addressed. The items below are
**new or residual** issues surfaced by reviewing the remediation itself. None block merge;
the most substantive is the partial M1 fix (F1).

## Major Issues (Should Fix)

_None._

## Minor Issues (Should Fix)

### 1. M1 fix is partial — selected guided command can still clip on short terminals (F1)
- **Source:** logic_reasoning_checker (verified against live code)
- **File:** `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs:331-426`
- **Problem:** `render_guided_commands` draws top-to-bottom from `area.y` with no scroll
  offset toward `selected_command_index`. When the guided section is clamped to a content
  area shorter than `guided_section_full_height()`, trailing command blocks are clipped from
  the bottom. At `selected_command_index = 2` on a short detail pane, the selected command
  and its inline `[c] copy` hint can fall off-screen while `c` still copies command 2 — the
  original M1 visible/copied divergence, now confined to short terminals.
- **Required Action:** Implement task-01 "option 2" — anchor a scroll window to
  `selected_command_index` so the selected command (label + command + `[c]` hint) is always
  within the rendered window when the section is space-constrained. Keep the saturating clamp.
- **Acceptance:** A regression test at a short detail height (e.g. 10-12 rows) with
  `selected_command_index = 2` asserts the selected command's text AND its `copy` hint are
  present in the rendered buffer; existing tall-terminal tests stay green; no panic on tiny
  terminals.

### 2. `LinuxPackageManager` bypasses the app re-export pattern in TUI test fixtures (F2)
- **Source:** architecture_enforcer
- **Files:** `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`,
  `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` (3 `#[cfg(test)]` sites);
  `docs/ARCHITECTURE.md` (~line 665, "four toolchain display types" note)
- **Problem:** The other four toolchain display types reach the TUI via
  `fdemon-app::install_wizard` re-exports; `LinuxPackageManager` (added to `ToolchainReport`
  by followup-04) is accessed directly via `fdemon_daemon::toolchain::`. No runtime violation
  (dev-dependency only), but it breaks the stated pattern and the ARCHITECTURE.md count is now
  stale (five types, not four).
- **Required Action:** Add `pub use fdemon_daemon::toolchain::LinuxPackageManager;` to
  `fdemon-app/src/install_wizard/mod.rs`; update the 3 TUI test sites to
  `fdemon_app::install_wizard::LinuxPackageManager`; correct the ARCHITECTURE.md note.
- **Acceptance:** No `fdemon_daemon::` path in `fdemon-tui` production or test source for this
  type; ARCHITECTURE.md note accurate; `cargo test --workspace` green.

### 3. Duplicated caption derivation — latent M1-clip drift hazard (F3)
- **Source:** code_quality_inspector, risks_tradeoffs_analyzer, logic_reasoning_checker
- **File:** `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` (height calc vs
  `render_guided_commands`, ~280-307 vs 346-369)
- **Problem:** `guided_section_full_height` and `render_guided_commands` each independently
  derive whether a step has a caption. They match exactly today, but a future captioned step
  kind updated in only one site silently desyncs reserved height from rendered rows,
  re-introducing an M1-style clip.
- **Required Action:** Extract a single source of truth, e.g.
  `fn step_caption(kind: WizardStepKind) -> Option<&'static str>` (or `step_has_caption`),
  and call it from both functions.
- **Acceptance:** One caption-deriving function; both call sites use it; existing
  `guided_section_full_height` unit tests (=12 for 3 commands) stay green.

## Minor Issues (Consider Fixing — NITPICK)

### 4. Redundant `missing_binaries.clone()` (N1)
- `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs:261` — move instead of clone.
  Not gate-failing (`redundant_clone` is nursery/allow-by-default), but contradicts the
  "avoid unnecessary clones" standard.

### 5. Best-effort caveat asymmetry across PM arms (N2)
- `crates/fdemon-app/src/install_wizard/state.rs:356-375` — add the "package names are
  best-effort; consult your distro docs" caveat to the dnf/pacman/zypper arms to match the
  yum arm, or document why only yum warranted it.

### 6. `winget_available: bool` vs `linux_package_manager: Option<…>` asymmetry (N3)
- `crates/fdemon-daemon/src/toolchain/types.rs` — optionally make symmetric
  (`Option<bool>`), or leave as-is (documented, single platform-gated consumer).

## Deferred / Tracked (Out of this followup's scope)

### D1. Windows `status == Ok` without VS C++ workload — real `vswhere.exe` probe
- The followup softened the detail text only; the status is still `Ok`, so status-branching
  consumers are still misled. This is the agreed Phase 4 interim. **Track as a future task**
  (Windows-only `vswhere.exe` probe that downgrades the status when the C++ workload is
  absent). Label m4 "mitigated/deferred", not "resolved".

## Re-review Checklist

After addressing F1-F3 (and optionally N1-N3):
- [ ] Selected guided command + `[c]` hint visible at a short detail height (F1 test)
- [ ] No `fdemon_daemon::` reach-through for `LinuxPackageManager` in `fdemon-tui` (F2)
- [ ] Single caption-deriving helper shared by height calc and renderer (F3)
- [ ] `cargo fmt --all -- --check` — clean
- [ ] `cargo check --workspace --all-targets` — clean
- [ ] `cargo test --workspace` — green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — clean
