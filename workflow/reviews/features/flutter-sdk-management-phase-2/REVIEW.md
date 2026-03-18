# Review: Flutter SDK Management — Phase 2 (Flutter Version Panel TUI)

**Review Date:** 2026-03-18
**Verdict:** NEEDS WORK
**Blocking Issues:** 3
**Files Changed:** 24 (10 new, 14 modified)
**Tests Added:** ~145 new tests across 4 crates

---

## Summary

Phase 2 implements a TUI panel for viewing and managing Flutter SDK versions. The implementation follows the project's established patterns (TEA, handler decomposition, widget overlay pattern) with comprehensive test coverage and clean layer boundaries. However, three blocking issues were identified by multiple reviewers: (1) the deletion safety check ignores `FVM_CACHE_PATH`, making the remove feature broken for users with custom cache paths; (2) `.fvmrc` writes overwrite the entire file, destroying user configuration — contradicting the PLAN.md specification; and (3) `dart_version` is not refreshed after a version switch, showing stale data. Several additional concerns warrant attention.

---

## Reviewer Verdicts

| Agent | Verdict | Critical | Major | Minor |
|-------|---------|----------|-------|-------|
| Architecture Enforcer | WARNING | 0 | 3 | 3 |
| Code Quality Inspector | NEEDS WORK | 0 | 1 | 6 |
| Logic Reasoning Checker | WARNING | 0 | 3 | 1 |
| Risks & Tradeoffs Analyzer | CONCERNS | 0 | 3 | 4 |

---

## Blocking Issues

### 1. FVM_CACHE_PATH mismatch between scanner and removal safety check

**Found by:** Architecture, Code Quality, Logic, Risks (all 4 reviewers)
**File:** `crates/fdemon-app/src/actions/mod.rs:778-782`

The cache scanner in `cache_scanner.rs` checks `FVM_CACHE_PATH` env var first, then falls back to `~/fvm/versions/`. The removal safety check in `actions/mod.rs` hardcodes `~/fvm/versions/` only. Users with `FVM_CACHE_PATH=/custom/path` can list versions but cannot delete them — the guard rejects all paths outside the hardcoded default.

Additionally, `dirs::home_dir().unwrap_or_default()` returns an empty `PathBuf` when `HOME` is unset, silently breaking the feature with a confusing error message.

### 2. `.fvmrc` overwrite destroys user configuration

**Found by:** Risks & Tradeoffs Analyzer
**File:** `crates/fdemon-app/src/actions/mod.rs:836-840`

`switch_flutter_version()` writes `{"flutter": "<version>"}`, completely replacing the file. FVM v3's `.fvmrc` supports additional fields (`flavors`, `runPubGetOnSdkChanges`, `updateVscodeSettings`, `updateGitIgnore`, `privilegedAccess`). The PLAN.md explicitly states "Read additional fields but don't modify them" — the implementation violates this specification.

### 3. `dart_version` not refreshed after version switch

**Found by:** Logic Reasoning Checker
**File:** `crates/fdemon-app/src/handler/flutter_version/actions.rs:90`

`handle_switch_completed` copies `state.resolved_sdk` into `sdk_info.resolved_sdk` but does NOT update `sdk_info.dart_version`. After switching versions, the SDK info pane displays the Dart version from the *original* SDK, not the new one.

---

## Major Issues (Should Fix)

### 4. `loading` state not set before scan begins

**Found by:** Architecture Enforcer
**File:** `crates/fdemon-app/src/handler/flutter_version/navigation.rs:21-27`

`handle_show()` calls `show_flutter_version()` which initializes `VersionListState::default()` where `loading: false`. The scan action is returned but hasn't completed yet. The UI briefly shows "No versions found" before the scan result arrives. `loading` should be set to `true` so the "Scanning..." state shows immediately.

### 5. No confirmation for destructive deletion

**Found by:** Risks & Tradeoffs Analyzer

Pressing `d` immediately triggers `remove_dir_all` on a Flutter SDK directory (2-3 GB). No confirmation prompt exists. The `d` key is adjacent to navigation keys (`j`/`k`), making accidental deletion likely.

### 6. Synchronous file read in `update()` cycle

**Found by:** Architecture Enforcer
**File:** `crates/fdemon-app/src/flutter_version/state.rs:35-50`

`FlutterVersionState::new()` calls `read_dart_version()` which does synchronous `std::fs::read_to_string()`. This runs inside the TEA `update()` cycle which should be fast and side-effect-free. The file is small (< 100 bytes), but this technically violates the TEA principle.

---

## Minor Issues

| # | Issue | File | Found By |
|---|-------|------|----------|
| 7 | `dirs` crate added to fdemon-tui for `format_path()` — platform call in TUI layer | `sdk_info.rs:201` | Architecture |
| 8 | `#[allow(dead_code)]` on unused `icons` field in 2 widgets | `sdk_info.rs:40`, `version_list.rs:42` | Code Quality |
| 9 | Magic percentage literals in `centered_rect` (Principle 4) | `mod.rs:90-105` | Code Quality |
| 10 | Unnecessary `.clone()` on `version` in `RemoveFlutterVersion` | `actions/mod.rs:801` | Code Quality |
| 11 | Stub handlers inline in `update.rs` instead of handler module | `update.rs:2474-2483` | Architecture |
| 12 | New `Cell<usize>` field not registered in `docs/REVIEW_FOCUS.md` | `state.rs:96` | Architecture |
| 13 | No stale scan result guard (rapid open/close sends multiple scans) | `navigation.rs` | Risks |
| 14 | `.fvmrc` JSON written with `format!` instead of `serde_json` | `actions/mod.rs:837` | Logic |

---

## Strengths

- Clean architectural decomposition following established patterns (handler/new_session/, widget/new_session_dialog/)
- Comprehensive test coverage: ~145 new tests across all layers (state, handlers, cache scanner, widgets)
- Correct Cell<usize> render-hint pattern with proper TEA exception annotations
- Named constants with derivation comments (CODE_STANDARDS Principle 4)
- Thorough sort logic with semver parsing, channel priority, and active-first ordering
- Render-time scroll clamping as independent safety net
- Good edge case handling (empty list, no SDK, loading/error states, tiny terminal)
- Proper use of `spawn_blocking` for all filesystem I/O

---

## Quality Gate Status

| Check | Result |
|-------|--------|
| `cargo fmt --all` | PASS |
| `cargo check --workspace` | PASS |
| `cargo clippy --workspace -- -D warnings` | PASS |
| `cargo test --workspace` | PASS (1770 passed, 1 pre-existing flaky TCP test) |

---

## Recommendation

Fix the 3 blocking issues and address major issues #4 (loading state) before merging. Issues #5 (deletion confirmation) and #6 (sync read) can be tracked as follow-ups if needed, but #5 is strongly recommended before production use.
