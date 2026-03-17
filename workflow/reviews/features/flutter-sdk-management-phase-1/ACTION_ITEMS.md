# Action Items: Flutter SDK Management Phase 1

**Review Date:** 2026-03-17
**Verdict:** :warning: NEEDS WORK
**Blocking Issues:** 1

---

## Critical Issues (Must Fix)

### 1. ToolAvailabilityChecked handler overwrites Flutter SDK fields

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/handler/update.rs:1178`
- **Problem:** `state.tool_availability = availability;` is a wholesale replacement that erases `flutter_sdk` and `flutter_sdk_source` fields set by `Engine::new()`. `ToolAvailability::check()` hardcodes these as `false`/`None`.
- **Required Action:** Preserve the Flutter SDK fields across the replacement. Either merge the incoming struct or save/restore the fields.
- **Acceptance:** Add a test that verifies `tool_availability.flutter_sdk == true` after processing `Message::ToolAvailabilityChecked` when `state.resolved_sdk` is `Some(...)`.

---

## Major Issues (Should Fix)

### 2. `find_flutter_sdk` function is ~430 lines

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-daemon/src/flutter_sdk/locator.rs:43-473`
- **Problem:** 10 near-identical strategy blocks repeat a validate/read-version/detect-channel/build pattern. Violates the 50-line function rule by 9x.
- **Suggested Action:** Extract a `try_strategy()` helper that takes `(sdk_root, source_builder)` and returns `Option<FlutterSdk>`. Each strategy block becomes 3-5 lines.

### 3. `read_version_file` `?` aborts entire detection chain

- **Source:** Risks & Tradeoffs Analyzer, Logic Checker
- **File:** `crates/fdemon-daemon/src/flutter_sdk/locator.rs` (lines 53, 88, 125, 167, 216, 258, 300, 342, 384, 424)
- **Problem:** If `validate_sdk_path` succeeds but `read_version_file` fails (permissions, race), the `?` operator propagates the error out of the entire function. Other valid strategies are never tried.
- **Suggested Action:** Replace `let version = read_version_file(&sdk_root)?;` with a match that logs the error and continues to the next strategy, consistent with how `validate_sdk_path` failures are handled.

### 4. Bare PATH fallback creates misleading FlutterSdk

- **Source:** All 4 agents
- **File:** `crates/fdemon-daemon/src/flutter_sdk/locator.rs:456-469`
- **Problem:** `FlutterSdk { root: PathBuf::from("flutter"), version: "unknown" }` violates the type's documented contract. `SdkSource::SystemPath` makes it indistinguishable from a properly resolved SDK.
- **Suggested Action:** Either (a) add a `SdkSource::BarePathFallback` variant, or (b) remove the fallback entirely -- `Engine::new()` already handles `Err(FlutterNotFound)` gracefully by continuing without an SDK.

---

## Minor Issues (Consider Fixing)

### 5. Fully-qualified Result type in version_managers.rs
- 7 functions use `fdemon_core::error::Result<Option<PathBuf>>` instead of bare `Result<Option<PathBuf>>` from the prelude import.

### 6. Missing tests for SdkResolved/SdkResolutionFailed
- Two simple handler tests needed per project testing standards.

### 7. Magic number 7 in channel.rs:64
- Replace with `const GIT_SHORT_HASH_LEN: usize = 7;`

### 8. PATH not restored in locator test
- `test_all_strategies_fail_returns_flutter_not_found` should save/restore PATH, not remove it.

### 9. Duplicate info! log for SDK resolution
- Logged in both `locator.rs` (structured) and `engine.rs` (format string). Remove the `engine.rs` duplicate.

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] Critical issue #1 resolved (ToolAvailabilityChecked preserves flutter fields)
- [ ] Test added verifying flutter_sdk fields survive ToolAvailabilityChecked
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings`
- [ ] Major issues #2-#4 resolved or justified with tracking issue
- [ ] Minor issues reviewed and addressed where reasonable
