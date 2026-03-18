# Review: Flutter SDK Management Phase 1

**Review Date:** 2026-03-17
**Feature:** Multi-Strategy SDK Locator
**Branch:** `feature/flutter-sdk-management`
**Task File:** `workflow/plans/features/flutter-sdk-management/phase-1/TASKS.md`
**Verdict:** :warning: **NEEDS WORK**

---

## Change Summary

Replaces hardcoded `Command::new("flutter")` with a 10-strategy SDK detection chain that resolves a `FlutterSdk` at startup. The resolved `FlutterExecutable` is threaded through all Flutter CLI interactions (process spawn, device discovery, emulator discovery/launch).

**Stats:** 35 files changed, 785 insertions, 205 deletions
**New module:** `crates/fdemon-daemon/src/flutter_sdk/` (5 files, ~2,700 lines)

---

## Agent Verdicts

| Agent | Verdict | Critical | Major | Minor |
|-------|---------|----------|-------|-------|
| Architecture Enforcer | PASS | 0 | 0 | 2 warnings |
| Code Quality Inspector | NEEDS WORK | 0 | 3 | 5 |
| Logic & Reasoning Checker | PASS | 0 | 0 | 3 warnings |
| Risks & Tradeoffs Analyzer | CONCERNS | 1 | 1 | 3 |

---

## Critical Issues

### 1. `ToolAvailabilityChecked` handler overwrites Flutter SDK fields

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/handler/update.rs:1178`
- **Severity:** CRITICAL (runtime correctness bug)

`ToolAvailability::check()` is async and returns a struct with `flutter_sdk: false` and `flutter_sdk_source: None` (hardcoded at `tool_availability.rs:78-79`). When the result arrives as `Message::ToolAvailabilityChecked`, the handler performs `state.tool_availability = availability;` -- a wholesale replacement that erases the Flutter SDK fields that `Engine::new()` populated moments earlier.

**Impact:** On every startup, `tool_availability.flutter_sdk` resets to `false` and `flutter_sdk_source` resets to `None` within seconds. Any UI or logic reading these fields will incorrectly report no SDK. The `state.resolved_sdk` field is unaffected, so session spawning still works, but the status display is wrong.

**Fix:** Preserve flutter_sdk fields across the replacement:
```rust
Message::ToolAvailabilityChecked { availability } => {
    let flutter_sdk = state.tool_availability.flutter_sdk;
    let flutter_sdk_source = state.tool_availability.flutter_sdk_source.clone();
    state.tool_availability = availability;
    state.tool_availability.flutter_sdk = flutter_sdk;
    state.tool_availability.flutter_sdk_source = flutter_sdk_source;
    // ...
}
```

---

## Major Issues

### 2. `find_flutter_sdk` is ~430 lines -- 9x the 50-line function limit

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-daemon/src/flutter_sdk/locator.rs:43-473`

The 10 strategy blocks repeat an identical 4-step pattern (call detector, validate, read version, build FlutterSdk) with only the strategy function and SdkSource variant varying. A shared helper would reduce the function from ~430 lines to ~80 lines.

### 3. `read_version_file` `?` propagation aborts entire detection chain

- **Source:** Risks & Tradeoffs Analyzer, Logic Checker
- **File:** `crates/fdemon-daemon/src/flutter_sdk/locator.rs` (10 sites)

After `validate_sdk_path` succeeds, `read_version_file(&sdk_root)?` uses `?` to propagate errors. If the VERSION file exists but is unreadable (permissions, race condition), the entire detection chain aborts instead of falling through to the next strategy. This is inconsistent with how `validate_sdk_path` failures are handled (which do fall through).

### 4. Bare PATH fallback creates a misleading `FlutterSdk`

- **Source:** All 4 agents flagged this
- **File:** `crates/fdemon-daemon/src/flutter_sdk/locator.rs:456-469`

When all 10 strategies fail but `flutter` is callable on PATH, a fallback creates `FlutterSdk { root: PathBuf::from("flutter"), version: "unknown" }`. This `root` is not a real directory, violating the documented contract. Uses `SdkSource::SystemPath` making it indistinguishable from a properly resolved PATH SDK.

---

## Minor Issues

### 5. Fully-qualified `fdemon_core::error::Result` in version_managers.rs

All 7 public detection functions use the fully-qualified path instead of the prelude's `Result` alias, inconsistent with the rest of the codebase.

### 6. Missing unit tests for `SdkResolved`/`SdkResolutionFailed` handlers

The two new message handlers in `update.rs` have no dedicated test coverage, despite project standards requiring tests for all state transitions.

### 7. Magic number `7` for git short hash in `channel.rs:64`

Should be a named constant per project standards.

### 8. `test_all_strategies_fail` removes PATH without restoration

At `locator.rs:640`, `std::env::remove_var("PATH")` removes PATH entirely instead of restoring the original value. Panics between set_var and remove_var would leave PATH unset.

### 9. Duplicate `info!` log for SDK resolution

SDK resolution is logged both inside `find_flutter_sdk` (locator.rs) and again in `Engine::new()` (engine.rs), producing duplicate log lines on every startup.

---

## Architecture Assessment

**Layer boundaries:** PASS -- All new code respects the dependency graph. `flutter_sdk` module correctly lives in `fdemon-daemon`, `fdemon-core` remains dependency-free, `fdemon-tui` imports only through `fdemon-app`.

**TEA compliance:** PASS with warnings -- SDK resolution in `Engine::new()` is synchronous filesystem I/O (acceptable for Phase 1). `FlutterExecutable` is correctly embedded in `UpdateAction` variants at creation time. `dispatch_spawn_session` reads state directly (pre-existing TEA bypass, now with an additional SDK dependency).

**Module placement:** PASS -- Types in `types.rs`, parsers in `version_managers.rs`, channel detection in `channel.rs`, orchestration in `locator.rs`, config in `fdemon-app/config/types.rs`.

---

## Testing Assessment

**Coverage:** Strong -- 60+ new tests across the flutter_sdk module, plus updates to ~20 existing tests to inject `fake_flutter_sdk()`.

**Gaps:**
- No tests for `SdkResolved`/`SdkResolutionFailed` handlers
- No test verifying `ToolAvailabilityChecked` preserves flutter_sdk fields (this would have caught the critical bug)
- PATH not properly restored in one test

**Env var isolation:** Correct use of `#[serial]` for env-var-modifying tests.

---

## Strengths

- Clean separation of concerns across the 5 new flutter_sdk module files
- Comprehensive version manager parser coverage (7 managers, 40+ tests)
- Graceful degradation when no SDK found -- fdemon still starts
- `fake_flutter_sdk()` test helper enables clean cross-crate testing
- Detection priority ordering is well-reasoned and clearly documented
- Windows `.bat` wrapper handling via `FlutterExecutable::WindowsBatch`
- `find_config_upward` enables monorepo support for all config-file strategies

---

## Recommendations

1. **Fix the ToolAvailabilityChecked overwrite bug** (blocking)
2. **Replace `?` with match-and-continue** for `read_version_file` calls in locator.rs
3. **Refactor `find_flutter_sdk`** to extract a `try_strategy` helper -- reduces ~430 lines to ~80
4. **Distinguish the bare PATH fallback** with `SdkSource::BarePathFallback` or remove it (Engine::new already handles FlutterNotFound gracefully)
5. **Add the 2 missing handler tests** + a test for ToolAvailabilityChecked field preservation
6. **Fix PATH restoration** in `test_all_strategies_fail_returns_flutter_not_found`
7. **Use bare `Result<>` alias** in version_managers.rs function signatures
8. **Remove duplicate info! log** (keep the structured log in locator.rs)
9. **Add `GIT_SHORT_HASH_LEN` constant** in channel.rs

---

## Blocking Issues: 1

The `ToolAvailabilityChecked` handler overwrite must be fixed before merge. See `ACTION_ITEMS.md` for the full remediation plan.
