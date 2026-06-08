# Review: Phase 4 Prerequisites — Followup Remediation

**Review Date:** 2026-06-04
**Branch:** `feat/toolchain-bootstrap`
**Diff base:** `004f4a2..HEAD` (7 commits `17811c3`→`8f5b950`)
**Reviews:** remediation of `workflow/reviews/features/phase-4-prereq/REVIEW.md` (the 2 MAJOR + 10 MINOR + 4 NITPICK original review)
**Method:** 5 specialized reviewers (architecture, quality, logic, risks, security) fanned out over the followup diff; the pivotal logic finding (W1, partial M1 fix) and the two quality "clippy" claims were adversarially re-verified against the live code by the orchestrator.
**Verdict:** ⚠️ **NEEDS WORK** (no blockers) — every original MAJOR/MINOR/NITPICK finding is addressed, but the M1 remediation is **partial** (selected-command clipping persists on short terminals) and several minor consistency/drift items were introduced. No CRITICAL, no security findings, no regressions; full quality gate is green.

## Dimension Verdicts

| Dimension | Verdict | Note |
|-----------|---------|------|
| security | ✅ APPROVED | All guided-command strings remain static literals; relocated `which` probes stay read-only; no new injection surface |
| logic | ⚠️ CONCERNS | **M1 fix is partial** — no scroll-to-selected; selected command can still clip on short terminals. All other remediations logically exact. |
| architecture | ⚠️ CONCERNS | TEA purity restored cleanly (followup-04 verified); minor: `LinuxPackageManager` reached via `fdemon_daemon::` directly in TUI test fixtures, bypassing the app re-export pattern |
| quality | ⚠️ CONCERNS | Doc/idiom fixes correct; duplicated caption derivation (drift hazard); one redundant clone (style, non-gating) |
| risks | ⚠️ CONCERNS | M2/followup-04 solid; m4 Windows remains a `status==Ok` false-green (deferred, not resolved); caveat asymmetry across PM arms |

## What the remediation got right (verified)

- **M1 row math** (`step_detail.rs`): `guided_section_full_height()` mirrors `render_guided_commands` exactly — identical `has_caption` derivation, `needs_blank = i > 0 || !has_caption`, per-command rows, header/caption. Saturating clamp `full_height.min(content_area.height)` preserved; the explicitly-**rejected** "simplify `bottom_area`" change was correctly **not** applied.
- **M2 Yum** (`state.rs:363`): emits a real `sudo yum install -y …` (no `dnf`); tests assert `contains("yum") && !contains("dnf")`. Package set is plausible for RHEL7/CentOS7 (EPEL assumption honestly disclosed in the caveat).
- **followup-04 TEA purity**: `prerequisites_guided_commands` is now a pure function of `ToolchainReport`; **zero** `which::` calls remain in `fdemon-app`; `which` dropped from `fdemon-app/Cargo.toml` + lock. Detection moved to async `run_preflight`, gated per platform (`Some(pm)`/`which("winget")` only on the applicable OS). Command output is byte-for-byte unchanged. The `r`-recheck path **recomputes** detection — no stale-detection risk.
- **m3 Missing/Partial**, **n1 GTK gating**, **m9 `detect_from_candidates`** precedence, **m5 doc accuracy**, **m6/m7/m8 tests** — all verified correct.
- **n3** typed-missing-keys hardening left as tracked `// TODO(phase-4-followup n3)` (state.rs:391, 431) — not silently dropped.
- **Quality gate**: `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` — all green (the only test failures were `/tmp` tmpfs-full linker SIGBUS in two untouched doctests; pass with `TMPDIR` on a non-full disk).

## Confirmed Findings (new / residual, verified against real code)

### 🟡 MINOR

**F1. M1 fix is partial — selected command (and its `[c]` copy hint) can still clip on short terminals; `c` copies an off-screen command.** `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs:331-426`. `render_guided_commands` draws commands top-to-bottom from `area.y` guarded by `y < area.y + area.height`, with **no scroll offset toward `selected_command_index`** (the index only drives styling/highlight). When `content_area.height < guided_section_full_height()`, the section is clamped, so trailing command blocks are clipped from the bottom. At `selected_command_index = 2` on a detail pane too short to fit all three macOS blocks (CLT + CocoaPods + Rosetta, ≈12 rows full), the Rosetta row + inline `[c] copy` fall outside `area` and are skipped — yet `c` still copies command 2. This is the original M1 visible/copied divergence, now confined to the short-terminal regime instead of all terminals. The regression tests only cover a comfortable 30-row case and an 8-row *no-panic* case that asserts nothing about selected-command visibility. The implementer chose task-01 "option 1" (size-to-count + clamp); "option 2" (anchor a scroll window to `selected_command_index`) is what fully satisfies "selected command always visible". [Source: logic_reasoning_checker; verified by orchestrator] → **followup-A**

**F2. `LinuxPackageManager` reached via `fdemon_daemon::toolchain::` directly in TUI test fixtures, bypassing the app re-export pattern.** `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` + `mod.rs` (3 `#[cfg(test)]` sites). The other four toolchain display types (`ComponentCheck`, `ComponentStatus`, `DoctorLine`, `DoctorMarker`) are re-exported through `fdemon-app::install_wizard` so the TUI never reaches into `fdemon-daemon` directly (`docs/ARCHITECTURE.md` "Note on daemon display types"). `LinuxPackageManager` (added to `ToolchainReport` by followup-04) is now a fifth such type but is accessed directly via the daemon path. No runtime violation (`fdemon-daemon` is `[dev-dependencies]` only), but it breaks the stated pattern, and the ARCHITECTURE.md "four toolchain display types" note is now stale (it's five). [Source: architecture_enforcer] → **followup-A**

**F3. Duplicated caption / `has_caption` derivation between `guided_section_full_height` and `render_guided_commands` — latent M1-clip drift hazard.** `step_detail.rs` (height calc ~280-307 vs renderer 346-369). Both independently derive whether a step has a caption via `matches!(kind, AndroidTools | Prerequisites)` / `caption_text.is_some()`. They match exactly today (no live bug), but a future captioned step kind updated in only one site would silently desync reserved height from rendered rows, re-introducing an M1-style clip. [Source: code_quality_inspector, risks_tradeoffs_analyzer, logic_reasoning_checker] → **followup-A**

### 🔵 NITPICK

**N1. Redundant `missing_binaries.clone()`.** `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs:261`. `missing_binaries` is owned and unused afterward — move instead of clone (per `docs/CODE_STANDARDS.md` "avoid unnecessary clones"). **Not gate-failing**: `clippy::redundant_clone` is a nursery lint (allow-by-default); a forced fresh `clippy --all-targets -- -D warnings` on `fdemon-daemon` is clean. [Source: code_quality_inspector; downgraded by orchestrator] → **followup-B (optional)**

**N2. Best-effort caveat added to the yum arm only.** `state.rs:356-375`. The dnf/pacman/zypper arms carry an `or: <apt>` cross-reference but not the new "package names are best-effort; consult your distro docs" caveat the yum arm now has. n2 flagged all non-apt arms; the fix left coverage asymmetric. [Source: risks_tradeoffs_analyzer] → **followup-B (optional)**

**N3. `winget_available: bool` vs `linux_package_manager: Option<…>` type asymmetry.** `crates/fdemon-daemon/src/toolchain/types.rs`. `false` conflates "probed, absent" with "not probed (non-Windows)"; documented in the field comment and harmless (single platform-gated consumer), but asymmetric with its sibling field. [Source: risks_tradeoffs_analyzer] → **followup-B (optional)**

### 🟠 Deferred / tracked (not new — agreed interim)

**D1. m4 Windows still reports `status == Ok` without the VS C++ workload.** `prerequisites.rs` (`build_windows_check_from_presence` Ok branch). The followup softened the **detail text** (`WINDOWS_MSVC_CAVEAT`) but the *status* is still `Ok`, so consumers branching on `status` (step rollup, green checkmark) still treat Windows prerequisites as satisfied. This is the agreed Phase 4 interim (a real `vswhere.exe` probe was explicitly deferred), but it should be tracked as a backlog item and labeled "mitigated/deferred", not "resolved". [Source: risks_tradeoffs_analyzer] → **track as future task (out of this followup's scope)**

## Rejected by Verification (NOT issues)

- **"`redundant_clone` / `collapsible_else_if` will fail `clippy -D warnings`"** (quality reviewer) — FALSE as a gate failure. Forced fresh `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` (16.45s, genuine re-lint) is clean. `redundant_clone` is allow-by-default (nursery); `collapsible_else_if` does not fire on `prerequisites.rs:227-231`. The clone remains a style NITPICK (N1) only.

## Quality Gate

`cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` — all green on the cumulative branch state (doctest SIGBUS failures were environmental: `/tmp` tmpfs at 100%; pass with `TMPDIR` redirected).

See `ACTION_ITEMS.md` for the actionable breakdown. Followup tasks (followup-A / followup-B) to be written by the planner.
