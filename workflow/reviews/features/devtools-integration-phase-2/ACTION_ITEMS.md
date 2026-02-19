# Action Items: Phase 2 — Flutter Service Extensions

**Review Date:** 2026-02-19
**Verdict:** :warning: NEEDS WORK
**Blocking Issues:** 5 (Critical: 2, Major: 3)

---

## Critical Issues (Must Fix)

### 1. Refactor VmServiceClient ownership in ObjectGroupManager/WidgetInspector

- **Source:** Logic Reasoning Checker, Risks Analyzer, Architecture Enforcer
- **File:** `crates/fdemon-daemon/src/vm_service/extensions.rs`
- **Lines:** 199-204 (ObjectGroupManager struct), 208 (new), 941-955 (WidgetInspector)
- **Problem:** `ObjectGroupManager` takes ownership of a `VmServiceClient` that is NOT `Clone` (contains `mpsc::Receiver`). `WidgetInspector` methods also take `&VmServiceClient` as a parameter, creating two separate client instances for one logical operation. The completion summaries incorrectly claim the client is cloneable. This design will not compile when Phase 4 integration code attempts real usage.
- **Required Action:** Change `ObjectGroupManager` to NOT own a `VmServiceClient`. Instead, pass `&VmServiceClient` to `create_group()`, `dispose_group()`, and `dispose_all()` as method parameters. Update `WidgetInspector` accordingly to thread the single client reference through all operations.
- **Acceptance:** `ObjectGroupManager` no longer has a `client` field. All methods that need the client receive it as a `&VmServiceClient` parameter. `WidgetInspector` uses one client reference consistently. Tests pass.

### 2. Split `extensions.rs` into submodules

- **Source:** All 4 reviewers
- **File:** `crates/fdemon-daemon/src/vm_service/extensions.rs` (1955 lines)
- **Problem:** File is 4x the 500-line guideline in `CODE_STANDARDS.md`. Contains 6+ distinct concerns. Will only grow worse in Phase 4.
- **Required Action:** Split into:
  ```
  vm_service/extensions/
  ├── mod.rs          — re-exports, ext constants module, build_extension_params, shared parsing helpers
  ├── overlays.rs     — DebugOverlayState, toggle_bool_extension, repaint_rainbow, debug_paint, etc.
  ├── inspector.rs    — ObjectGroupManager, WidgetInspector, get_root_widget_tree, etc.
  ├── layout.rs       — extract_layout_info, parse_widget_size, extract_layout_tree, fetch_layout_data
  └── dumps.rs        — DebugDumpKind, debug_dump, debug_dump_app/render/layer
  ```
- **Acceptance:** No single file exceeds 500 lines. All existing tests pass. All re-exports in `vm_service/mod.rs` still work. `cargo clippy --workspace -- -D warnings` clean.

---

## Major Issues (Should Fix)

### 3. Fix silent error swallowing in `get_root_widget_tree` fallback

- **Source:** All 4 reviewers
- **File:** `crates/fdemon-daemon/src/vm_service/extensions.rs`
- **Line:** 664-666
- **Problem:** `Err(_)` discards ALL errors from the newer API before falling back. Transport errors, channel closures, and timeouts all silently trigger a redundant fallback call.
- **Suggested Action:** Replace `Err(_)` with logic that checks if the error indicates "extension not available" before falling back. For transport errors, propagate immediately:
  ```rust
  Err(e) => {
      // Only fall back if the newer method is not registered on this Flutter version.
      // Check if the raw JSON error matches the "extension not available" pattern.
      // For non-protocol errors (ChannelClosed, Daemon), propagate immediately.
      tracing::debug!("getRootWidgetTree failed, attempting fallback: {e}");
      // ... fallback logic for method-not-found only ...
  }
  ```
- **Acceptance:** Transport errors propagate immediately. "Method not found" errors trigger fallback. The discarded error is logged at debug level.

### 4. Remove unused `_client` parameter from `dispose_all`

- **Source:** All 4 reviewers
- **File:** `crates/fdemon-daemon/src/vm_service/extensions.rs`
- **Line:** 287
- **Problem:** `_client` parameter is completely unused — the method uses `self.client` via `dispose_group()`. The `WidgetInspector::dispose` at line 1028 propagates this unused parameter.
- **Suggested Action:** Remove the parameter from `dispose_all`. Update `WidgetInspector::dispose` signature and call site.
- **Acceptance:** `dispose_all` has no `_client` parameter. `WidgetInspector::dispose` has no `client` parameter (unless needed for other reasons after fixing issue #1). Tests pass.

### 5. Fix `create_group` error-path state loss

- **Source:** Logic Reasoning Checker, Risks Analyzer
- **File:** `crates/fdemon-daemon/src/vm_service/extensions.rs`
- **Lines:** 226-233
- **Problem:** `self.active_group.take()` clears the old group name BEFORE `dispose_group()`. If dispose fails, the old group name is lost, `active_group` is `None`, and the old group is leaked on the Flutter side.
- **Suggested Action:** Either (a) log the dispose failure and proceed with creating the new group regardless, or (b) restore the old group name on failure. Option (a) is simpler and aligns with the doc comment ("non-fatal in most cases"):
  ```rust
  pub async fn create_group(&mut self) -> Result<String> {
      if let Some(old) = self.active_group.take() {
          if let Err(e) = self.dispose_group(&old).await {
              tracing::warn!("Failed to dispose group '{}': {e}", old);
              // Proceed anyway — old group is leaked but new work can continue
          }
      }
      self.group_counter += 1;
      let name = format!("fdemon-inspector-{}", self.group_counter);
      self.active_group = Some(name.clone());
      Ok(name)
  }
  ```
- **Acceptance:** `create_group` succeeds even when the old group dispose fails. A warning is logged. Tests cover the failure path.

---

## Minor Issues (Consider Fixing)

### 6. Replace `RwLock::unwrap()` with poison-safe access in `client.rs`

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-daemon/src/vm_service/client.rs:162,227,243,424,437,443,457,466,483`
- **Problem:** 9 `unwrap()` calls on `RwLock` in production code paths. Panics if lock is poisoned.
- **Suggested Action:** Use `unwrap_or_else(|e| e.into_inner())` to clear poison and continue.
- **Note:** These are pre-existing from Phase 1. Fix while the file is being modified.

### 7. Fix `parse_optional_diagnostics_node_response` logic inconsistency

- **Source:** Code Quality Inspector, Architecture Enforcer
- **File:** `crates/fdemon-daemon/src/vm_service/extensions.rs:623-628`
- **Problem:** Computes `node_value` but passes original `value` to `parse_diagnostics_node_response`. Works by coincidence (both functions extract `result`). Fragile if either changes.
- **Suggested Action:** Use `node_value` directly instead of delegating to `parse_diagnostics_node_response`.

### 8. Add `EXTENSION_NOT_AVAILABLE_CODE` constant for magic number 113

- **Source:** Code Quality Inspector, Risks Analyzer
- **File:** `crates/fdemon-daemon/src/vm_service/extensions.rs:157`
- **Suggested Action:** Add `const EXTENSION_NOT_AVAILABLE_CODE: i32 = 113;` alongside existing `METHOD_NOT_FOUND_CODE`.

### 9. Fix `query_all_overlays` documentation or make it concurrent

- **Source:** Code Quality Inspector, Risks Analyzer
- **File:** `crates/fdemon-daemon/src/vm_service/extensions.rs:438-448`
- **Problem:** Doc says "concurrently" but implementation is sequential (`.await` between struct fields).
- **Suggested Action:** Either fix doc to say "sequentially" or use `tokio::join!`.

### 10. Add `tracing::warn!` for silently skipped children in `extract_layout_tree`

- **Source:** Logic Reasoning Checker, Risks Analyzer, Architecture Enforcer
- **File:** `crates/fdemon-daemon/src/vm_service/extensions.rs:862-868`
- **Suggested Action:** Log when a child fails deserialization.

### 11. Add `Serialize`/`Deserialize` to `LayoutInfo`, `BoxConstraints`, `WidgetSize`

- **Source:** Architecture Enforcer, Risks Analyzer
- **File:** `crates/fdemon-core/src/widget_tree.rs`
- **Problem:** Inconsistent with `DiagnosticsNode` which has serde derives. Will block NDJSON/state serialization in Phase 4.
- **Suggested Action:** Add `#[derive(Serialize, Deserialize)]` to these types.

### 12. Clarify `visible_node_count()` doc comment semantics

- **Source:** Logic Reasoning Checker
- **File:** `crates/fdemon-core/src/widget_tree.rs:117-127`
- **Suggested Action:** Explicitly document that hidden parents exclude their entire subtree.

---

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] All critical issues (1-2) resolved
- [ ] All major issues (3-5) resolved or justified
- [ ] `cargo fmt --all` — clean
- [ ] `cargo check --workspace` — clean
- [ ] `cargo test --lib` — all pass
- [ ] `cargo clippy --workspace -- -D warnings` — zero warnings
- [ ] No single file exceeds 500 lines (per CODE_STANDARDS.md)
- [ ] `VmServiceClient` is used consistently (no dual-client pattern)
- [ ] `dispose_all` has no unused parameters
