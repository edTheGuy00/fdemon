# Code Review: Toolchain Platforms Submenu — Phase 1 (Reorder PATH after Flutter SDK)

**Review Date:** 2026-06-09
**Change Type:** Feature (Phase 1 of multi-phase feature)
**Branch:** `feat/toolchain-platforms-submenu`
**Diff Base:** `git diff 479f120..HEAD` (merge commits `5274b70`, `253fa60`)
**Verdict:** ✅ **APPROVED**

---

## Scope

Pure display reorder of the install-wizard steps: swap `PathConfig` and `FlutterSdk` in `build_steps()` so the order becomes `Prerequisites[0] → AndroidTools[1] → FlutterSdk[2] → PathConfig[3] → Doctor[4]`. No new types, no behavior change, no rename of `AndroidTools` (that lands in Phase 2).

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/install_wizard/state.rs` | `build_steps()` vec order swap (FlutterSdk/PathConfig blocks), doc-comment + grouping bullets, 3 index-based tests |
| `crates/fdemon-app/src/handler/install_wizard/actions.rs` | Soft-tip string reword + 20 index-based test value changes (FlutterSdk 3→2, PathConfig 2→3) |
| `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | 1 test fixture + 2 tests + 4 comment-only annotation updates |
| `website/src/pages/docs/toolchain.rs` | ASCII-art row swap, numbered-table rows 3/4 swap with descriptions, "Step order vs. install order" info-box reword |

---

## Agent Verdicts

| Agent | Verdict | Findings |
|-------|---------|----------|
| `architecture_enforcer` | ✅ APPROVED | 0 violations; 2 cosmetic suggestions (no action) |
| `code_quality_inspector` | ✅ APPROVED | 0 critical/major; 2 minor, 1 nitpick |
| `logic_reasoning_checker` | ✅ APPROVED | 0 issues; every test index cross-checked against its assertion |
| `risks_tradeoffs_analyzer` | ✅ APPROVED | 0 blocking; 2 LOW tech-debt notes for Phase 2 |
| `security_reviewer` | ✅ APPROVED | 0 findings (all severities) |

**Overall:** All agents ✅ → **APPROVED**.

---

## Key Confirmations

- **Single source of truth preserved.** `build_steps()` remains the sole ordering authority. All production consumers access steps via `selected_step()`, `position(|s| s.kind == …)`, or kind-keyed `match` arms — none encode a hardcoded index. The only index literals are in tests, all updated.
- **No order-dependent logic touched.** `match WizardStepKind` arms, the completion/auto-configure chain (`FlutterSdk → AutoConfigurePath → PathConfig → RerunPreflight`), and `path_config_status` (reads the `flutter_sdk` component bucket, not a vec position) are all order-independent and unchanged.
- **"Install Flutter first" gate retained** (`actions.rs` `bin_dir == None` arm) and still reachable via manual nav to PathConfig before any SDK resolves. Not a dead path.
- **Tests are defensively anchored.** Each renumbered test pairs `selected_index = N` with a `assert_eq!(selected_step().kind, …)` precondition (or a kind-specific behavior assertion), so a stale index fails loudly rather than passing against the wrong step. `test_empty_step_shows_no_components_message` and the FlutterSdk-dispatch tests are the strongest guards.
- **Implementor correctly updated 20 index sites, not the 13 named in the task brief.** The extra 7 (Phase-5 async/token tests) were genuinely broken by the reorder and not in the "leave untouched" list — updating them was correct (confirmed by `logic_reasoning_checker`).
- **No security impact.** Execution order is TEA-message-driven and kind-keyed, not display-order-driven; no rc-file-write, download, or PATH logic changed; guided-command strings are static copy-paste literals, never executed.
- **Website docs internally consistent** with the new order; the info-box no longer claims PATH precedes Flutter SDK. The reorder is a net UX improvement (UI order now mirrors install/dependency order).

---

## Tracked Minor Findings (non-blocking)

### MINOR-1 — Stale module-header bullet in `step_detail.rs` (pre-existing)
[Source: code_quality_inspector]
The `//!` module header groups `Prerequisites` with `Doctor` as showing "Available in a later phase", but `Prerequisites` now renders a guided-command block in most real states. **Pre-existing staleness, not introduced by this change**, but more visible now. Consider splitting the bullet to describe both sub-cases. Out of scope for this task (which marked the file as comment-only).

### LOW-1 — Hardcoded-index test pattern is fragile for Phase 2
[Source: risks_tradeoffs_analyzer, code_quality_inspector]
~20 tests encode positions as bare integer literals. Phase 2 (AndroidTools → Platforms submenu) will shift indices again and force another manual renumber, with a silent-mis-target risk (e.g. FlutterSdk@2 and PathConfig@3 both have 0 guided commands, so a mis-pointed noop test still passes). **Recommendation:** in Phase 2, migrate executable-step tests from literal `selected_index = N` to `position(|s| s.kind == WizardStepKind::X)` — the pattern already used by the order-independent `select_step()` tests — rather than renumbering literals again.

### LOW-2 — No invariant test locks "PathConfig immediately follows FlutterSdk"
[Source: risks_tradeoffs_analyzer]
The auto-configure chain is kind-keyed (order-tolerant), so this is an intent/documentation gap, not a runtime risk. **Optional:** add `assert!(pathconfig_idx == flutter_idx + 1)` when touching this area in Phase 2.

---

## Verification

- Both worktrees passed full gate independently: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --lib` (1487 tests).
- Post-merge spot check on combined tree: `cargo test --workspace --lib install_wizard` → 124 passed, 0 failed.

## Documentation Freshness

No stale project docs. No new modules/crates/build-steps/dependencies/patterns introduced. `docs/ARCHITECTURE.md` describes crate/file layout (not wizard step order) and correctly needs no change. Website docs were updated as part of this change. ✅

---

## Recommendation

**Approve and proceed to Phase 2.** Carry the LOW-1 test-migration recommendation into the Phase 2 task plan so the AndroidTools→Platforms rename does a kind-lookup migration instead of another literal renumber.
