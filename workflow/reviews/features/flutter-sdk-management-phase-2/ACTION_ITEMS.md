# Action Items: Flutter SDK Management — Phase 2

**Review Date:** 2026-03-18
**Verdict:** NEEDS WORK
**Blocking Issues:** 3

## Critical Issues (Must Fix)

### 1. Fix FVM_CACHE_PATH mismatch in removal safety check
- **Source:** All 4 reviewers
- **File:** `crates/fdemon-app/src/actions/mod.rs:778-782`
- **Problem:** Safety check hardcodes `~/fvm/versions/` while scanner uses `FVM_CACHE_PATH` env var. Users with custom cache paths can list but not delete versions. Also, `dirs::home_dir().unwrap_or_default()` silently produces an empty path when HOME is unset.
- **Required Action:**
  1. Make `resolve_fvm_cache_path()` in `cache_scanner.rs` public (or extract to a shared location)
  2. Use the same resolution in the `RemoveFlutterVersion` action dispatcher
  3. Replace `unwrap_or_default()` with `ok_or_else(|| Error::config("Cannot determine home directory"))?`
- **Acceptance:** Version removal works correctly when `FVM_CACHE_PATH` is set to a custom path

### 2. Implement JSON merge for .fvmrc writes
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/actions/mod.rs:836-840` (`switch_flutter_version` function)
- **Problem:** `.fvmrc` is overwritten with `{"flutter": "<version>"}`, destroying any existing FVM configuration fields. PLAN.md specifies "Read additional fields but don't modify them."
- **Required Action:**
  1. Read existing `.fvmrc` file if it exists
  2. Parse as `serde_json::Value`
  3. Set/update only the `"flutter"` field
  4. Write back the complete JSON object
  5. Fall back to minimal write if file does not exist or is not valid JSON
- **Acceptance:** Switching versions preserves existing `.fvmrc` fields (`flavors`, `runPubGetOnSdkChanges`, etc.)

### 3. Fix dart_version not refreshed after version switch
- **Source:** Logic Reasoning Checker
- **File:** `crates/fdemon-app/src/handler/flutter_version/actions.rs:86-94`
- **Problem:** `handle_switch_completed` updates `sdk_info.resolved_sdk` but not `sdk_info.dart_version`. After switching SDKs, the panel shows the Dart version from the original SDK.
- **Required Action:** After updating `sdk_info.resolved_sdk`, also update `sdk_info.dart_version` by calling the existing `read_dart_version()` function (or a re-exported version of it) on the new SDK root.
- **Acceptance:** After switching versions, the SDK info pane shows the correct Dart version for the newly active SDK

## Major Issues (Should Fix)

### 4. Set loading state before scan begins
- **Source:** Architecture Enforcer
- **File:** `crates/fdemon-app/src/handler/flutter_version/navigation.rs:21-27`
- **Problem:** Panel opens with `loading: false`, briefly showing "No versions found" before scan completes.
- **Suggested Action:** In `handle_show()`, after `state.show_flutter_version()`, set `state.flutter_version_state.version_list.loading = true`
- Alternatively, set `loading: true` in `FlutterVersionState::new()`

### 5. Add deletion confirmation
- **Source:** Risks & Tradeoffs Analyzer
- **Problem:** `d` key immediately deletes a 2-3 GB SDK directory with no confirmation. Adjacent to navigation keys.
- **Suggested Action:** Implement double-press pattern: first `d` sets a pending-delete marker + status message "Press d again to remove X"; second `d` within timeout confirms. Or use the existing `ConfirmDialog` infrastructure.

## Minor Issues (Consider Fixing)

### 6. Move `format_path()` to fdemon-app to remove `dirs` from fdemon-tui
- Store pre-formatted path string in `SdkInfoState`, remove `dirs` dependency from `fdemon-tui`

### 7. Remove `#[allow(dead_code)]` on unused `icons` fields
- `sdk_info.rs:40-43` and `version_list.rs:42-43` — remove field, add back in Phase 3

### 8. Name the dialog percentage constants in `centered_rect`
- `mod.rs:90-105` — define `DIALOG_HEIGHT_PERCENT`, `DIALOG_WIDTH_PERCENT` per Principle 4

### 9. Register new Cell<usize> field in docs/REVIEW_FOCUS.md
- `VersionListState::last_known_visible_height` must be documented per project standard

### 10. Route stub handlers through handler functions
- Move `FlutterVersionInstall`/`FlutterVersionUpdate` stubs from `update.rs` into the handler module

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] All 3 critical issues resolved
- [ ] Major issue #4 (loading state) resolved
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes
- [ ] Version removal works with `FVM_CACHE_PATH` set to a custom path
- [ ] Switching versions preserves existing `.fvmrc` fields
- [ ] Dart version updates correctly in SDK info pane after version switch
- [ ] Panel shows "Scanning..." immediately on open (not "No versions found")
