# Code Review: Phase 5 — Runnable-Device Filtering

**Review Date:** 2026-05-30
**Branch:** `feat/ux-polish-and-multilaunch`
**Diff Base:** `3f92fd0..HEAD` (commits `c29fd83`, `38fa830`, `641bfea`)
**Change Type:** Feature implementation (3 tasks)
**Overall Verdict:** ⚠️ **NEEDS WORK**

---

## Summary

Phase 5 stops the dialog from offering devices the Flutter toolchain won't run for the
current project. It adds `is_supported` (+ `capabilities`) to the daemon `Device` type,
filters explicitly-unsupported connected devices at a single shared chokepoint
(`group_connected_devices`), guards multi-select so unsupported devices can never be
checked, and adds an actionable empty-state message.

The core design — **filter once, in `group_connected_devices`** — is well-reasoned and
correctly implemented: the flat list, cursor, checked-set, and TUI click-regions all
derive from the same filtered view, so there are no index-alignment defects. Layer
boundaries and TEA purity are respected. The conservative `default = true` for absent
flags is the right call.

Two issues hold this back from a clean approval: (1) a **MAJOR** test gap — the test named
to guard the `toggle_checked_cursor` unsupported path never calls that function; and
(2) a **HIGH/MAJOR** consistency gap — the non-interactive launch paths
(`find_auto_launch_target`, headless, cached selection) ignore `is_supported`, so the
"never launch an unrunnable target" invariant holds in the dialog but breaks exactly where
no empty-state message can warn the user. The scope note in `TASKS.md` ("Connected tab
only") arguably makes this by-design, but it is undocumented as a deliberate boundary.

## Agent Verdicts

| Agent | Verdict | Headline |
|-------|---------|----------|
| architecture_enforcer | ✅ PASS | 0 violations; single-chokepoint design verified |
| code_quality_inspector | ⚠️ APPROVED WITH RESERVATIONS | 1 MAJOR (test doesn't exercise function under test) |
| logic_reasoning_checker | ✅ PASS | Index alignment correct; no logic defects |
| risks_tradeoffs_analyzer | ⚠️ CONCERNS | 1 HIGH (auto-launch bypass), 1 MEDIUM (silent partial hiding) |
| security_reviewer | ✅ PASS | 0 critical/high; narrow surface (trusted local subprocess) |

## Acceptance Criteria

All six Phase 5 success criteria are met and tested (parsing present-true/false/absent,
filtering, multi-select skip, two-way empty state, quality gate green). Verified
independently by `logic_reasoning_checker` against each task's criteria.

---

## Findings

### 🟠 MAJOR

#### M1. Test `toggle_checked_cursor_skips_unsupported` never calls `toggle_checked_cursor`
**[Source: code_quality_inspector, architecture_enforcer, logic_reasoning_checker]**
`crates/fdemon-app/src/new_session_dialog/target_selector_state.rs:872-884`

The test asserts only on the private helper `is_connected_device_supported`; the actual
guard inside `toggle_checked_cursor` (line ~396) is never executed. The test name promises
coverage that does not exist. The guard is provably defensive (the cursor can't resolve to
an unsupported id once filtered from the flat list), so this is a coverage gap rather than
a correctness defect — but the test should exercise the production path. Suggested fix:
pre-seed `checked_device_ids`, call `toggle_checked_cursor()`, assert the set is unmodified
(requires `let mut state`).

#### M2. Non-interactive launch paths bypass the `is_supported` filter
**[Source: risks_tradeoffs_analyzer]**
`crates/fdemon-app/src/spawn.rs:242-267` (`find_auto_launch_target` and its tiers:
`try_auto_start_config`, `try_cached_selection`, `try_first_config`, `bare_flutter_run`);
headless path at `src/headless/runner.rs:~300`.

These resolve a device with **no `is_supported` check**. With `auto_start = true`, a cached
`last_device`, or headless mode, fdemon can still launch a device Flutter reports as
unsupported — and these are precisely the paths where the empty-state message can't rescue
the user; the launch fails later at the Flutter layer. `TASKS.md` scopes Phase 5 to the
"Connected tab only," which may make this acceptable for this phase, but the boundary is
**undocumented** and the policy now lives in the view layer while the data property lives
on `Device`, inviting future drift. **Decision required:** is the invariant dialog-only or
system-wide? Either document the scope boundary explicitly, or apply a default-true
fallback filter in `find_auto_launch_target` (+ tests) and extract a shared
`Device::is_runnable()` so the two paths can't diverge.

### 🟡 MINOR

#### m1. Silent partial-set hiding (mixed supported/unsupported) [Source: risks_tradeoffs_analyzer]
`crates/fdemon-tui/.../device_list.rs:212-228` — The empty-state message fires only when the
*entire* filtered list is empty. In the common mixed case (e.g., Chrome hidden because web
is disabled, alongside a working emulator), the unsupported device vanishes with zero
breadcrumb. Consider a "(N hidden: not runnable for this project)" footer. UX trade-off —
track, not blocking.

#### m2. `is_connected_device_supported` — prefer `.any()` [Source: code_quality_inspector]
`target_selector_state.rs:465-471` — `.find().map().unwrap_or(false)` is equivalent to the
more idiomatic `self.connected_devices.iter().any(|d| d.id == id && d.is_supported)`.

#### m3. `toggle_select_all` double-collects `Vec` then `BTreeSet` [Source: code_quality_inspector]
`target_selector_state.rs:416-421` — Collect directly into `BTreeSet<String>` and test
`all_checked` over it, avoiding the intermediate `Vec` + second `into_iter().collect()`.

#### m4. `DeviceCapabilities` not re-exported from `fdemon-daemon/src/lib.rs` [Source: code_quality_inspector, architecture_enforcer]
Declared `pub` but omitted from the `pub use devices::{...}` block and module doc. Task 01
explicitly marks the export optional this phase; reconcile (export + document, or note as
internal) before a future phase consumes `capabilities`.

#### m5. `debug!` logs full subprocess stdout [Source: security_reviewer]
`crates/fdemon-daemon/src/devices.rs:238` — Pre-existing; the new `capabilities` data
increases what's captured. Consider demoting full payload to `trace!` and logging a summary
at `debug`. Low real-world exposure (local file log).

#### m6. Defensive indexing in `device_groups.rs` [Source: security_reviewer]
Lines 266/286/309 use `selectable[...]` after an `is_empty()` guard. Safe today; prefer
`.last()` / `.get(i)` to encode the invariant structurally against future refactors.

#### m7. `Device` has no `Default` → ~27 struct-literal updates [Source: risks_tradeoffs_analyzer]
Each new `Device` field forces a workspace-wide literal sweep (a recurring merge-conflict
tax). Consider a test-only builder or `#[derive(Default)]` with a sensible platform default.

### 🔵 NITPICK

- **n1.** `capabilities` is parse-and-stored but unused this phase (and `Serialize`d). Acceptable as forward-compat; remove if no consumer lands within ~2 phases. [risks_tradeoffs_analyzer]
- **n2.** Scoped `use ratatui::...` inside the empty-state block in `device_list.rs` is inconsistent with top-of-file imports elsewhere. [code_quality_inspector]
- **n3.** `cached_flat_list.as_ref().unwrap()` (`target_selector_state.rs:162`) is provably safe but bare; `get_or_insert_with` removes the `unwrap`. Pre-existing. [security_reviewer]
- **n4.** Em-dash written as `\u{2014}` in the empty-state literal; the literal `—` reads cleaner. [security_reviewer]

---

## Documentation Freshness

✅ No stale docs. No new modules/crates, no dependency or build-command changes, no new
conventions. `TASKS.md` explicitly records "No core-doc update required" for a struct field
+ dialog filter. **However**, if M2 is resolved by *documenting* the dialog-only scope, the
natural home is a short note in `docs/REVIEW_FOCUS.md` (or the plan) recording that
`is_supported` filtering is intentionally dialog-scoped and `find_auto_launch_target` is
exempt — so the divergence isn't later "fixed" as a bug.

## Recommendation

Address **M1** (trivial test fix) and make an explicit **M2** decision (document the scope
boundary, or extend the filter to the auto-launch path with tests). The MINOR items are
good cleanups but non-blocking. See `ACTION_ITEMS.md`.
