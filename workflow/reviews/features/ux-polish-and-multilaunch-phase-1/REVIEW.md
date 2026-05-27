# Code Review: Phase 1 — Multi-Device Launch Picker

**Feature:** ux-polish-and-multilaunch / phase-1-multi-launch
**Review Date:** 2026-05-27
**Diff Base:** `b21d488..HEAD` (`feat/ux-polish-and-multilaunch`, excludes `workflow/plans/`)
**Reviewers:** architecture_enforcer, code_quality_inspector, logic_reasoning_checker, risks_tradeoffs_analyzer, security_reviewer

## Overall Verdict: ⚠️ NEEDS WORK

The feature is well-architected, TEA-compliant, and broadly well-tested — zero-checked behavior is a faithful no-regression of the legacy single-launch path, the multi-select state model is clean and well-covered, and the checkbox rendering is sound. The verdict is NEEDS WORK (not REJECTED) because three of five reviewers raised converging concerns on the **multi-launch fan-out failure path**, and one factual error in the code's own comment masks a real (if low-frequency) resource leak with a one-line fix.

Nothing here crashes or corrupts state. The blocking items are a session-slot leak and a missing test for a named acceptance criterion (AC#4).

### Agent Verdicts

| Agent | Verdict |
|-------|---------|
| architecture_enforcer | ✅ PASS |
| code_quality_inspector | ⚠️ NEEDS WORK |
| logic_reasoning_checker | ⚠️ PASS WITH CONCERNS |
| risks_tradeoffs_analyzer | ⚠️ CONCERNS (track follow-ups) |
| security_reviewer | ✅ PASS (advisory findings) |

## Consolidated Findings

### 🟠 MAJOR

**M1. Orphaned session leak in `spawn_one` — comment is factually wrong**
`crates/fdemon-app/src/handler/new_session/launch_context.rs:705–714`
[Source: code_quality_inspector, logic_reasoning_checker, risks_tradeoffs_analyzer, security_reviewer]

When `spawn_one` returns `Err` after `create_session_*` has already inserted the session (no-SDK branch), the session is left in the manager. The inline comment (line 707) claims *"There is no undo API — ... it will be garbage-collected by the capacity eviction."* **Both halves are false** (verified):
- `SessionManager::remove_session` **does exist** (`session_manager.rs:201`).
- `evict_oldest_stopped` (`session_manager.rs:163`) only reclaims `AppPhase::Stopped` sessions. An orphan sits in `Initializing` forever, so it is **never** garbage-collected. It occupies one of the 9 slots and `find_active_by_device_id` treats it as active, blocking relaunch of that device until fdemon restarts.

Under multi-launch the leak is amplified (potentially several orphans per confirm).

**Fix:** Call `state.session_manager.remove_session(session_id)` on the post-create `Err` path before returning, OR hoist the `flutter_executable()` check to before `create_session_*` (preferred — fail fast, zero orphans). Correct the misleading comment.

**M2. No test for AC#4 (session-cap hit mid-loop)**
`crates/fdemon-app/src/handler/new_session/launch_context.rs` (tests)
[Source: logic_reasoning_checker, risks_tradeoffs_analyzer, task_validator (orchestration)]

AC#4 ("cap hit mid-loop → return already-built actions + 'launched X of Y' toast, no panic") is the most complex path and has **no dedicated test**. The logic is currently *correct only by a fragile, undocumented invariant*: in-loop sessions are `Initializing` and thus not evictable, so `create_session_*` returns `Err` cleanly. A future change making eviction more aggressive could strand an already-pushed action referencing a removed `session_id`.

**Fix:** Add a test seeding ~8 active sessions + ≥2 checked devices; assert partial `actions.len()`, one skip, `ui_mode == Normal`, warn toast present, no panic. The seeding pattern already exists at `launch_context.rs:2147`.

### 🟡 MINOR

**m3. Document the eviction-policy coupling in `spawn_one`**
`launch_context.rs` / `session_manager.rs:163`
[Source: logic_reasoning_checker]
Add a comment noting that returning already-built actions on a mid-loop cap hit is only safe because `evict_oldest_stopped` never touches the `Initializing` sessions created earlier in the loop. Prevents a silent dangling-id regression if the eviction policy ever changes.

**m4. `save_last_selection` may persist a skipped device**
`launch_context.rs` (primary = `i == 0`)
[Source: logic_reasoning_checker, risks_tradeoffs_analyzer]
The auto-launch default is persisted for device index 0 unconditionally. If device 0 is skipped (already active) but device 1 launches, the persisted default points at a device that never launched. Persist for the first *successfully launched* device instead.

**m5. Unbounded N-way concurrent process spawn (no throttling)**
`launch_context.rs` → `actions_vec` → fire-and-forget `tokio::spawn`
[Source: risks_tradeoffs_analyzer]
Confirming up to 9 checked devices launches up to 9 `flutter run` processes near-simultaneously (Gradle/Xcode builds, VM Service sockets, native-log tasks). Acceptable for Phase 1, but document the resource expectation and track a future staggered-spawn enhancement.

**m6. Strip ANSI from `device.name`/`reason` in toast & error strings**
`launch_context.rs:570–583`
[Source: security_reviewer]
Device names originate from `flutter devices` daemon stdout. Apply the existing `fdemon_daemon::flutter_sdk::diagnostics::strip_ansi()` before rendering in toasts/errors, consistent with existing user-facing display code. Defense-in-depth for a local tool.

**m7. `extra_args` has no format/length validation (pre-existing, amplified)**
`config/launch.rs:333–334`, `launch_context.rs:617–640`
[Source: security_reviewer — rated HIGH; contextualized to MINOR here]
`extra_args` from `.fdemon/launch.toml` / `.vscode/launch.json` are passed verbatim to `Command::args()`. No shell injection risk (args are separate, not shell-evaluated), and the trust zone is the local developer who already controls the project dir. The multi-launch fan-out multiplies the blast radius (e.g. `--dart-define-from-file=../../x.env` to N processes). Consider a lax allowlist (starts with `--`, no NUL, max length) at config load. Not blocking given the local-developer trust model.

### 🔵 NITPICK / Pre-existing (not introduced by this PR)

- **n8.** `auto_save_if_fdemon_config` logic duplicated ~6× in `launch_context.rs` (pre-existing handlers). Extract a private helper. [code_quality_inspector]
- **n9.** `calculate_scroll_offset` duplicated between `target_selector_state.rs:464` (app) and `device_list.rs` (tui) — pre-existing `TODO`, candidate for `fdemon-core`. [architecture_enforcer, code_quality_inspector]
- **n10.** Magic `VISIBLE_ITEMS` constants (7, 10) in `state.rs` lack derivation comments. [code_quality_inspector]
- **n11.** Inline `std::collections::HashSet` full-path in `target_selector_state.rs:243`; import at top for consistency. [code_quality_inspector]
- **n12.** Pre-app shared-source double-trigger under fan-out (per-`spawn_one` snapshot). Pre-existing limitation, amplified; track in pre-app-custom-sources backlog. [architecture_enforcer, risks_tradeoffs_analyzer]
- **n13.** Redundant `ui_mode = Normal` write after `hide_new_session_dialog()` (harmless). [logic_reasoning_checker]
- **n14.** Redundant `set_dart_defines` call before `close_dart_defines_modal_with_changes()`. [code_quality_inspector]
- **n15.** `debug_assert!` modal-double-open guards compiled out in release; prefer early-return. Pre-existing. [security_reviewer]
- **n16.** `flat_list()` `unwrap()` lacks `// Safety` justification / `get_or_insert_with`. Pre-existing. [security_reviewer]
- **n17.** `reset_scroll` is `pub` with no external caller. [code_quality_inspector]

## Security Summary

No critical vulnerabilities. Trust model is appropriate for a local developer tool: all launch inputs originate from the local user and reach `Command::args()` as separate (non-shell-evaluated) elements — no command injection. Advisory items: `extra_args` validation (m7), ANSI stripping of daemon-sourced device names (m6).

## Architecture Summary

Clean PASS. TEA fully respected — state mutations confined to `fdemon-app`, view is a pure read (the `with_checked(&BTreeSet)` borrow), side effects routed through `UpdateResult::actions_vec`, new messages dispatched through `handler::update`. No layer-boundary violations. No new `Cell` render-hint fields (existing `last_known_visible_height` reused per the approved exception).

## Documentation Freshness

- `docs/KEYBINDINGS.md` — ✅ updated (Space / `a`).
- No new modules/crates/deps → `ARCHITECTURE.md` / `DEVELOPMENT.md` unaffected.
- Recommend a short user-facing note on multi-launch resource expectations (relates to m5).

## Quality Gates (integrated branch, verified during orchestration)

`fmt` ✅ · `check --all-targets` ✅ · `test --workspace` ✅ (no failures) · `clippy -D warnings` ✅

## Recommendation

Address **M1** (one-line `remove_session` rollback + comment fix — verified API exists) and **M2** (AC#4 test) before considering Phase 1 merge-ready. The MINOR items (m3–m7) should be tracked; m3/m4 are cheap and worth folding into the M1/M2 fix. NITPICKs are largely pre-existing and can be batched into a separate cleanup task.
