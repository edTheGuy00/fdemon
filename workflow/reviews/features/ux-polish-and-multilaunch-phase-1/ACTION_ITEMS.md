# Action Items: Phase 1 — Multi-Device Launch Picker

**Review Date:** 2026-05-27
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 2 (M1, M2)

## Major Issues (Should Fix Before Merge)

### 1. Fix orphaned-session leak in `spawn_one` (and the false comment)
- **Source:** code_quality_inspector, logic_reasoning_checker, risks_tradeoffs_analyzer, security_reviewer
- **File:** `crates/fdemon-app/src/handler/new_session/launch_context.rs:705–714`
- **Problem:** On the no-SDK `Err` path, a session already inserted by `create_session_*` is left in the manager. The comment claims "no undo API" and "garbage-collected by capacity eviction" — **both false**: `SessionManager::remove_session` exists (`session_manager.rs:201`), and `evict_oldest_stopped` only reclaims `Stopped` sessions, so an `Initializing` orphan is never reclaimed and permanently consumes a slot (blocks device relaunch). Amplified under multi-launch.
- **Required Action:** Either (preferred) hoist the `flutter_executable()` check to *before* `create_session_*` so no session is created on the no-SDK path; or call `state.session_manager.remove_session(session_id)` before returning `Err`. Delete/correct the misleading comment.
- **Acceptance:** A test that triggers the post-create failure path asserts `session_manager` session count is unchanged (no orphan) and the device id is relaunchable afterward.

### 2. Add the missing AC#4 test (cap hit mid-loop)
- **Source:** logic_reasoning_checker, risks_tradeoffs_analyzer, task_validator
- **File:** `crates/fdemon-app/src/handler/new_session/launch_context.rs` (test module; pattern at line 2147)
- **Problem:** AC#4 (cap hit mid-loop → return built actions + "launched X of Y" toast, no panic) is the most complex path and has no dedicated test. Current correctness depends on an undocumented eviction invariant.
- **Required Action:** Seed ~8 active sessions + ≥2 checked devices; assert partial `actions.len()`, one skip, `ui_mode == Normal`, warn toast present, no panic.
- **Acceptance:** Test passes and fails if eviction behavior regresses.

## Minor Issues (Track; m3/m4 cheap enough to fold into the above)

### 3. Document eviction-policy coupling in `spawn_one`
- **Source:** logic_reasoning_checker
- **Action:** Comment that returning already-built actions on a mid-loop cap hit is safe only because `evict_oldest_stopped` never evicts the `Initializing` sessions created earlier in the loop.

### 4. Persist `save_last_selection` for the first *successfully launched* device
- **Source:** logic_reasoning_checker, risks_tradeoffs_analyzer
- **Action:** Avoid persisting an auto-launch default that points at a skipped device (currently keyed to `i == 0`).

### 5. Strip ANSI from `device.name` / `reason` in toast & error strings
- **Source:** security_reviewer
- **File:** `launch_context.rs:570–583`
- **Action:** Apply `fdemon_daemon::flutter_sdk::diagnostics::strip_ansi()` to daemon-sourced strings before user-facing display.

### 6. Document multi-launch resource expectation; consider staggered spawn
- **Source:** risks_tradeoffs_analyzer
- **Action:** Note in user docs that confirming N checked devices launches up to N concurrent `flutter run` builds. Track a future staggered/throttled-spawn enhancement.

### 7. Consider `extra_args` validation at config load
- **Source:** security_reviewer (HIGH in isolation; MINOR under local-developer trust model)
- **Action:** Optional lax allowlist (starts with `--`, no NUL, max length) in `config/launch.rs`. Not blocking.

## UI Improvement (Follow-up — requested 2026-05-27)

### 8. Remove redundant platform-letter icon prefix from device rows
- **Source:** user request
- **Files:** `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs`
  - Connected list: `icon_prefix` built at `device_list.rs:170` from `device_icon()` (`:28`)
  - Bootable list: `prefix` built at `device_list.rs:380` from `bootable_device_icon()` (`:47`)
  - Glyph source: `IconMode::Unicode` in `crates/fdemon-tui/src/theme/icons.rs` — `[M]` smartphone (`:41`), `[W]` globe (`:48`), `[D]` monitor (`:55`)
- **Problem:** The `[M]` / `[W]` / `[D]` platform-letter icons are redundant with the group headers (`IOS DEVICES`, `WEB`, `DESKTOP`) that already label each platform group. Combined with the new `[ ]` multi-select checkbox, they consume horizontal width that could go to the device name.
- **Required Action:** Remove the platform icon prefix from device rows in **both** the connected device list and the bootable device list. Drop the `icon_prefix` / `prefix` span (and its `device_icon` / `bootable_device_icon` call) from row rendering, and remove `icon_prefix.len()` from the `reserved` width calc so name truncation reclaims the freed columns. Decide whether `device_icon` / `bootable_device_icon` (and the `IconSet` wiring through `with_icons`) become dead code and remove them if so.
- **Open decisions:**
  - Whether removal applies to all icon modes or only `IconMode::Unicode` (NerdFonts mode renders nicer glyph icons). Recommendation: remove in all modes since the redundancy with the group header is mode-independent; revisit if NerdFonts users object.
  - Whether the group header is always present (it is the thing that makes the icon redundant). Confirm headers render in every layout variant (full + compact) before removing.
- **Acceptance:** No platform-letter/icon prefix renders on connected or bootable device rows; the checkbox (`[ ]`/`[x]`) and device name shift left to reclaim the width; name truncation accounts for the removed prefix; widget tests updated to assert the icon glyph is absent and the checkbox/name positions are correct; `cargo test --workspace` and `cargo clippy --workspace` pass.

## Pre-existing Cleanup (separate task, non-blocking)

- Extract `auto_save_if_fdemon_config` helper (~6 duplicated blocks in `launch_context.rs`).
- Deduplicate `calculate_scroll_offset` (app + tui copies) into `fdemon-core` — existing `TODO`.
- Derivation comments for `VISIBLE_ITEMS` constants; top-level `HashSet` import; `reset_scroll` visibility; redundant `set_dart_defines`/`ui_mode` writes; `debug_assert!` → early-return; `flat_list()` `unwrap()` justification.

## Re-review Checklist

- [ ] M1 resolved — no orphaned session on failure path; comment corrected; verified by test
- [ ] M2 resolved — AC#4 cap-hit-mid-loop test added and passing
- [ ] m3, m4 addressed or explicitly deferred
- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass
