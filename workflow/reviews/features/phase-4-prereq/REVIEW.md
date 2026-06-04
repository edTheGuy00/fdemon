# Review: Phase 4 — OS Prerequisites (toolchain-bootstrap)

**Review Date:** 2026-06-04
**Branch:** `feat/toolchain-bootstrap`
**Diff base:** `129e66e..HEAD` (6 commits `f8a3dce`→`32cd4da`)
**Method:** 8 specialized reviewers (architecture, quality, logic×3, security, risks, tests-docs) fanned out via workflow, each finding adversarially verified by an independent skeptic before inclusion.
**Verdict:** ⚠️ **NEEDS WORK** — 2 MAJOR (non-blocking crash-wise, but real correctness/UX defects) + 10 MINOR + 4 NITPICK. No CRITICAL, no security findings. All core detection/navigation logic verified correct.

## Dimension Verdicts

| Dimension | Verdict | Note |
|-----------|---------|------|
| logic-daemon | ✅ APPROVED | Precedence, ARCH gating, status mapping, parse round-trip all correct |
| logic-app | ✅ APPROVED | Guided-command emission, index clamping, all 3 resets, Enter split correct |
| security | ✅ APPROVED | All command strings static literals; probes use `Stdio::null`, no injection |
| architecture | ⚠️ CONCERNS | `which::which` I/O inside `update()` (TEA purity) |
| quality | ⚠️ CONCERNS | `Missing:`/`missing:` prefix drift; import inconsistency |
| logic-tui | ⚠️ CONCERNS | **Multi-command guided section clips all but first command** |
| risks | ⚠️ CONCERNS | Yum→dnf command; Windows VS C++ false-Ok |
| tests-docs | ⚠️ CONCERNS | Missing `[`/`]` tests; misleading test names; stale ARCHITECTURE.md |

## Confirmed Findings (verified against real code)

### 🟠 MAJOR

**M1. Multi-command Prerequisites clips all but the first command; `c` copies an off-screen command.**
`crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs:470-524`. When a Prerequisites step has component checks **and** multiple guided commands (the real macOS path: CLT + CocoaPods + Rosetta), `bottom_section_height` reserves a fixed 6 rows regardless of terminal height. Commands at index ≥1 are entirely clipped, so pressing `]` moves the selection (and the `c` copy target) to a command whose highlight and `[c]` hint are off-screen — visible/copied diverge. No panic (writes are bounds-guarded). The 3-command test masks it by setting `components: vec![]`, routing through the un-capped full-area branch. → **followup-01**

**M2. Yum branch emits a `dnf` command that fails on the only systems that reach it.**
`crates/fdemon-app/src/install_wizard/state.rs:352-356`. `detect_linux_package_manager` reaches the `Yum` arm only when `dnf` is absent (apt→dnf→yum precedence) — i.e. legacy yum-only RHEL7/CentOS7 — yet the arm emits `sudo dnf install …` under a "(yum)" label with no substitution note. The command fails with `dnf: command not found` on exactly the platform that reaches it. → **followup-02**

### 🟡 MINOR

**m1. `which::which` filesystem I/O inside the TEA `update()` path.** `state.rs:340,432` — `prerequisites_guided_commands` (called from `apply_report`←`handle_preflight_completed`) runs up to 6 `which::which` PATH probes synchronously. `update()` is contractually pure; only `Cell` render-hints and `version_check` network I/O are approved exceptions. → **followup-04**

**m2. Linux detail uses `Missing: ` (capital-M), diverging from the `MISSING_PREFIX` (`missing: `) contract.** `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs:219,490`. Latent only (Linux path doesn't call `parse_missing_prereq_keys` today, and the contract is doc-scoped to macOS/Windows), but an ad-hoc string mimicking the documented contract is a maintainability landmine. → **followup-03**

**m3. Linux reports `Partial` for absent hard-required tools; macOS/Windows report `Missing`.** `prerequisites.rs:216-221`. Cross-platform status divergence vs the documented `ComponentStatus` semantics (`Partial` = "present but degraded"). Harmless today (both roll up to non-Ok), but misleads future consumers branching on `Missing` vs `Partial`. → **followup-03**

**m4. Windows reports `Ok` without the VS "Desktop development with C++" workload (false-Ok).** `prerequisites.rs:414-461`. Gates only on git; VS C++ detection deferred (note-only). A user with git but no MSVC toolchain sees `Ok` and hits an opaque build failure later. Interim: soften the Ok detail text. → **followup-03**

**m5. `GUIDED_COMMAND_MIN_HEIGHT` doc-comment misdescribes the rendered rows.** `step_detail.rs:66-70`. Claims "1 blank + label + command + copy hint = 4", but the `[c]` hint shares the command row and the leading blank is skipped under a caption. Constant value is still a safe minimum; the derivation misleads anyone editing the (buggy, see M1) reservation math. → **followup-01**

**m6. PREREQ_KEY_GIT referenced via full crate path instead of the import block.** `state.rs:426` — peer constants are imported by short name; this one isn't. (Same issue flagged by two dimensions.) Becomes moot if followup-04 moves winget detection to the daemon. → **followup-04**

**m7. Missing key-mapping tests for `[` and `]` in `handle_key_install_wizard`.** `crates/fdemon-app/src/handler/keys.rs:444-445`. Every sibling binding (`Enter`/`Esc`/`Tab`/`r`/`c`) has a test; these two don't. → **followup-05**

**m8. `test_non_android_steps_have_no_guided_commands` asserts a now-false invariant.** `state.rs:1093-1104`. Passes only because the fixture has no Prerequisites/Git component; the Prerequisites step *can* now carry guided commands. Misleading name + vacuous coverage. → **followup-05**

**m9. `test_package_manager_precedence_apt_before_dnf` makes no assertion.** `prerequisites.rs:504-516`. Calls the fn and discards the result; precedence is untested. Module comment even falsely claims a "pure helper" exists. → **followup-05**

**m10. ARCHITECTURE.md module-table entries stale for Phase 4.** Lines 353 (`state.rs`), 357-358 (`navigation.rs`, `actions.rs`) track per-phase additions but stop at Phase 3. → **followup-06**

### 🔵 NITPICK

**n1. GTK dev-header absence double-reported when `pkg-config` itself is missing.** `prerequisites.rs:203-206`. Cosmetic over-report (install command covers both anyway); GTK presence is genuinely undeterminable without pkg-config. → **followup-03 (optional)**

**n2. Community-sourced dnf/pacman/zypper package names carry wrong-package risk with no best-effort caveat.** `state.rs:347-366`. Copy-paste, user-reviewed; each arm has an `or: <apt>` note but no "best-effort" caveat. → **followup-02 (optional)**

**n3. Stringly-typed detail format is a cross-crate parse contract.** `prerequisites.rs:47-87`. Well-mitigated (single `MISSING_PREFIX`, shared keys, round-trip tests). Future hardening: surface missing keys as a typed field on `ComponentCheck`. → **followup-04 (deferred note)**

**n4. `which` added as an `fdemon-app` dependency.** `crates/fdemon-app/Cargo.toml`. Not a layer violation (external crate; daemon already uses it). Resolved naturally if followup-04 moves PATH probing to the daemon. → **followup-04**

## Rejected by Adversarial Verification (NOT issues)

- **"`bottom_area` height calc is convoluted / could use `bottom_section_height` directly"** — FALSE. Under saturating arithmetic the current form computes `min(B, H)`, intentionally clamping `bottom_area` to the content region; the "simplification" would draw an out-of-bounds Rect on small terminals.
- **"`is_jdk_actionable` lacks a `///` doc comment"** — FALSE. It already has a full `///` doc block (and is Phase 3 code, out of scope).

## Quality Gate
`cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` — all green (0 test failures) on the cumulative branch state.

See `tasks/` (written by planner) for the actionable followup breakdown.
