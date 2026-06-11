# Review: Toolchain Platforms Submenu — Phase 5 (Windows leaf: vswhere detection + guided commands)

**Review Date:** 2026-06-10
**Diff Range:** `21a9cd8920bd2845e7118921fb1ecc47ac84c78f..4d9eb360` (HEAD at review time)
**Verdict:** ⚠️ **APPROVED WITH CONCERNS**

## Scope

Phase 5 graduates the Windows platform leaf from its Phase-2 inert placeholder into a live
detect + guided-only step. Six tasks: daemon vswhere two-gate probe + `ComponentKind::VisualStudioCpp`
(01), guided-only handler arm (02), `build_steps` bucketing + Missing→Partial cap +
`windows_guided_commands` (03), TUI caption + coming-soon suppression (04), ARCHITECTURE.md (05),
website toolchain page rewrite (06).

**Files reviewed:** 7 Rust source files (1,234 insertions), `docs/ARCHITECTURE.md`,
`website/src/pages/docs/toolchain.rs`.

## Agent Verdicts

| Agent | Verdict |
|-------|---------|
| architecture_enforcer | ⚠️ CONCERNS (0 critical, 1 warning, 2 suggestions) |
| code_quality_inspector | ⚠️ APPROVED WITH MINOR CONCERNS (0 major, 4 minor, 2 nitpicks) |
| logic_reasoning_checker | ✅ PASS (all 7 invariants verified; 2 non-blocking notes) |
| risks_tradeoffs_analyzer | ✅ Acceptable with tracked concerns (0 blocking) |
| security_reviewer | ✅ PASS (0 critical, 0 high, 3 medium, 2 low) |

**Severity floor applied:** no agent produced a Critical or Major finding; every agent explicitly
marked its findings non-blocking. Verdict caps at APPROVED WITH CONCERNS.

## What Is Solid

- **Layer discipline is clean** — probe lives entirely in `fdemon-daemon`; app applies the
  non-blocking Missing→Partial policy; TUI only reads state. No boundary violations.
- **Pure-classifier strategy is correct and well-executed** — `classify_vswhere_gates` covers
  gate-2-hit / gate-1-only / both-miss / empty / malformed JSON / over-long names with fixture
  tests that run on Linux CI; live-probe assertions are `#[cfg(target_os = "windows")]`-gated.
- **All seven phase invariants verified by logic trace**: two-gate classification, host gating
  (empty off-Windows, Unknown→Unknown, one real check on Windows), bucket routing + cap +
  Pending-on-empty, modify/winget/choco branching, handback never blocked
  (`flutter_now_live` reads only FlutterSdk), guided-only handler arm (no `begin_step`),
  no dual-CTA in the TUI.
- **Subprocess hardening complete**: `PROBE_TIMEOUT`, `.kill_on_drop(true)`,
  `stdin(Stdio::null())`, `strip_and_truncate`, never panics; vswhere JSON deserialized into a
  minimal struct; no probe output is ever interpolated into command strings.
- Docs and component counts (10 Linux / 11 Windows / 12 macOS) consistent across code and docs.

## Findings (deduplicated)

### Major

None.

### Medium (tracked, non-blocking)

#### M1. `VS_FOUND_PREFIX` cross-crate contract held by string duplication, not a shared constant
[Source: architecture_enforcer, code_quality_inspector, risks_tradeoffs_analyzer, security_reviewer, logic_reasoning_checker]
- **Files:** `crates/fdemon-daemon/src/toolchain/checks/windows.rs:42` (`pub(crate) const VS_FOUND_PREFIX`),
  `crates/fdemon-app/src/install_wizard/state.rs:~37` (duplicate `WINDOWS_VS_FOUND_PREFIX`)
- **Problem:** `pub(crate)` blocks the cross-crate import, so the app duplicates the literal. No
  compiler or test fails if the strings drift; drift silently drops the "modify existing VS"
  guided entry (fresh-install branch shown instead). Deviates from the project's `PREREQ_KEY_*`
  pattern (pub + re-export chain, imported by `state.rs`). Task 01's notes even contradicted
  themselves on this (`pub(crate)` "so Task 03 can reference it" — it can't).
- **Fix:** promote to `pub`, re-export via `checks/mod.rs` → `toolchain/mod.rs` → `lib.rs`
  (same path as `PREREQ_KEY_XCODE_CLT`), import in `state.rs`, delete the duplicate. Low cost.

#### M2. Modify-VS guided command is cmd.exe-only syntax shown without a shell caveat
[Source: code_quality_inspector, security_reviewer]
- **File:** `crates/fdemon-app/src/install_wizard/state.rs` (`windows_guided_commands`, modify entry)
- **Problem:** `start "" "%ProgramFiles(x86)%\…\setup.exe"` does not expand in PowerShell — the
  default shell on modern Windows — so the copy-paste silently fails for most users on that path.
- **Fix:** branch on `report.shell` (PowerShell: `Start-Process …` with `$env:` syntax) or append
  a "run in Command Prompt (cmd.exe)" note.

#### M3. Sequential vswhere gates double worst-case probe latency vs the iOS template
[Source: architecture_enforcer, risks_tradeoffs_analyzer, logic_reasoning_checker]
- **File:** `crates/fdemon-daemon/src/toolchain/checks/windows.rs:152–166`
- **Problem:** gate 1 and gate 2 are independent queries but awaited serially → worst case
  2 × `PROBE_TIMEOUT` (20s) stalls the whole preflight fan-out on a degraded host. `check_ios`
  runs independent gates via `tokio::join!`.
- **Fix:** `tokio::join!` the two `run_vswhere` futures (the classifier already takes both at once).

### Minor

- **N1.** Missing test: VS-found detail + `winget_available: false` → exactly `[modify, choco]`
  (`state.rs`). [code_quality_inspector]
- **N2.** `test_step_detail_windows_without_guided_commands_shows_no_dual_cta` doesn't assert
  "coming soon" / "flutter doctor" actually render (iOS twin does). [code_quality_inspector]
- **N3.** TUI test helper sets the Windows leaf to `StepStatus::Missing`, a state `build_steps`
  can never produce (always capped to Partial). Use `Partial`. [architecture_enforcer]
- **N4.** Redundant triple `strip_and_truncate` in `classify_vswhere_gates` (lines 202–204); the
  single-call pattern from `ios.rs` suffices. [code_quality_inspector]
- **N5.** Test re-declares `MAX_DETAIL_LEN: usize = 256` instead of using `super::MAX_DETAIL_LEN`.
  [code_quality_inspector]
- **N6.** choco command emitted unconditionally (no `choco_available` signal); note text mitigates.
  Accepted as-is. [risks_tradeoffs_analyzer]
- **N7.** Website ASCII wizard mock shows a macOS-host view (4 leaves) with no host-gating note;
  iOS `xcodes` note condensed. (Carried from Task 06 validator CONCERN.) [task_validator]
- **N8.** Security LOWs, accepted: `%ProgramFiles(x86)%`/PATH binary-planting surface is identical
  to every other tool probe (developer tooling, not a security boundary); `from_utf8_lossy` and
  spawn-error→"not found" conflation are pre-existing probe patterns. [security_reviewer]

## Documentation Freshness

`docs/ARCHITECTURE.md` and the website toolchain page were updated **within the phase**
(tasks 05/06) and verified against shipped code by the task validators. No stale-doc findings.
No new config fields, keybindings, or build steps → CONFIGURATION/KEYBINDINGS/DEVELOPMENT need
no changes (confirmed by doc_maintainer).

## Pre-existing Issues (not caused by this phase)

- `toolchain::tests::test_run_preflight_nonexistent_sdk_path_does_not_panic` fails at
  `toolchain/mod.rs:374` (asserts `Ok != Ok`). Reproduced at the phase base commit; classified
  PRE-EXISTING by both integration verifiers. Should be fixed independently.

## Verification

- Wave 2 + Wave 4 integration verifiers: `cargo fmt --check`, `cargo check --workspace
  --all-targets`, `cargo test --workspace` (4764 passed, 1 pre-existing failure),
  `cargo clippy --workspace --all-targets -- -D warnings`, website `cargo check` — all PASS.

## Recommendation

Merge as-is. File a small Phase-5 followup (or fold into the next phase) covering M1–M3 + N1–N5;
none block. M1 is the one with real silent-failure potential and is a ~5-line fix.
