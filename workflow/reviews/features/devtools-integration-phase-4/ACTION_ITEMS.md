# Action Items: DevTools Integration Phase 4

**Review Date:** 2026-02-19
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 3

---

## Critical Issues (Must Fix)

### 1. Loading state stuck forever when VM not connected

- **Source:** Logic Reasoning Checker, Risks Analyzer, VM Bug Investigation
- **Files:** `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/process.rs`
- **Problem:** `RequestWidgetTree` unconditionally sets `loading = true` in the handler,
  then the hydration function in `process.rs` silently discards the action (`returns None`)
  when no VM handle is available. No `WidgetTreeFetchFailed` message is sent back, so
  `inspector.loading` stays `true` forever. Same pattern affects `RequestLayoutData`.
- **Required Action:**
  1. In `update.rs`: Guard `loading = true` behind a `vm_connected` check on the active session
  2. In `process.rs`: When hydration discards an action, send a failure message back
     (e.g., `Message::WidgetTreeFetchFailed` with "VM not connected" error)
  3. Apply the same fix pattern to `RequestLayoutData` / `LayoutDataFetchFailed`
- **Acceptance:** Pressing `r` in Inspector when VM is not connected shows an error
  message instead of spinning "Loading…" forever

### 2. VM Service connection silently blocked / failure invisible in DevTools

- **Source:** VM Bug Investigation (Codebase Researcher)
- **Files:** `crates/fdemon-daemon/src/session.rs` (~line 221), `crates/fdemon-app/src/handler/update.rs`
- **Problem:** Two sub-issues:
  (a) `maybe_connect_vm_service` has a `vm_shutdown_tx.is_none()` guard that can
  permanently block new connections if stale shutdown state is not cleaned up after
  disconnect.
  (b) `VmServiceConnectionFailed` only writes a warning to session logs. If the user
  is in DevTools mode, they never see the failure — the Performance panel just shows
  "VM Service not connected" with no indication that connection was attempted and failed.
- **Required Action:**
  1. Audit `vm_shutdown_tx` lifecycle — ensure it is set to `None` when VM service
     disconnects or the session is recycled
  2. On `VmServiceConnectionFailed`, update `DevToolsViewState` with an error field
     that the Performance panel can display (e.g., "Connection failed: <reason>")
  3. Consider adding a retry mechanism or a manual "Connect" button in the UI
- **Acceptance:** When VM Service fails to connect, the DevTools panels display the
  failure reason. A new session or reconnect attempt is not blocked by stale state.

### 3. TEA violation — side effect in handler (`open_url_in_browser`)

- **Source:** Architecture Enforcer
- **File:** `crates/fdemon-app/src/handler/devtools.rs`
- **Line:** `handle_open_browser_devtools` function
- **Problem:** Calls `std::process::Command::new(...).spawn()` directly inside the
  update handler, violating the TEA pattern. All side effects must be returned as
  `UpdateAction` variants and executed in the action dispatch layer (`actions.rs`).
- **Required Action:**
  1. Add `UpdateAction::OpenBrowserDevTools { url: String }` variant
  2. Move the `open_url_in_browser` logic to `actions.rs`
  3. Have the handler return the action instead of executing the side effect
- **Acceptance:** `handle_open_browser_devtools` returns an `UpdateResult::action(…)`
  with no direct I/O. Browser launch happens in `actions.rs`.

---

## Major Issues (Should Fix)

### 1. No DevTools state reset on session switch

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/state.rs`
- **Problem:** `devtools_view_state` is a single global field on `AppState`. When the
  user switches sessions (tabs 1-9), the inspector tree, loading state, layout data,
  and error messages from the previous session remain visible.
- **Suggested Action:** Either make `DevToolsViewState` per-session (stored on
  `SessionHandle`) or reset it when the active session changes. The reset approach is
  simpler and sufficient for now.

### 2. `visible_nodes()` allocates Vec on every render frame

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/state.rs` — `InspectorState::visible_nodes()`
- **Problem:** Builds a `Vec<(&DiagnosticsNode, usize)>` via recursive tree traversal
  on each call. Called during `Widget::render()`, which runs every frame (~60 FPS
  potential).
- **Suggested Action:** Cache the visible nodes list and invalidate on tree change or
  expand/collapse toggle. A simple `Option<Vec<…>>` with dirty flag would suffice.

### 3. Inline VM Service RPC bypasses VmServiceClient

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/actions.rs`
- **Problem:** `spawn_fetch_widget_tree`, `spawn_fetch_layout_data`, and
  `spawn_toggle_overlay` construct raw JSON-RPC calls via `VmRequestHandle::call_extension()`
  instead of using `VmServiceClient` methods. This creates dual maintenance.
- **Suggested Action:** Add corresponding methods to `VmServiceClient` in
  `fdemon-daemon` and call those from `actions.rs`.

### 4. No VM object group disposal

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/actions.rs` — `spawn_fetch_widget_tree`
- **Problem:** Widget tree fetches allocate object groups on the VM but never call
  `disposeGroup`. Long-running sessions will leak memory on the VM side.
- **Suggested Action:** Track the current object group name in `InspectorState` and
  dispose the previous group before fetching a new tree.

---

## Minor Issues (Consider Fixing)

### 1. Duplicate `truncate_str()` helper
- `inspector.rs` and `layout_explorer.rs` both define identical `truncate_str()` functions.
  Extract to `crates/fdemon-tui/src/widgets/devtools/mod.rs` or a shared utility.

### 2. `render_tab_bar` visibility
- `DevToolsView::render_tab_bar` is `pub` but only called from `Widget::render()`.
  Should be `pub(crate)` or private.

### 3. `percent_encode_uri` hex casing
- Produces `%2f` (lowercase) instead of `%2F` (uppercase). RFC 3986 recommends uppercase.

### 4. Debug overlay toggle debounce
- Rapid key presses can fire multiple overlay toggle RPCs before the first completes.
  Consider debouncing or ignoring while a toggle is in-flight.

### 5. Layout panel `object_id` vs `value_id`
- `RequestLayoutData` passes `object_id` to the VM Service. Some Flutter versions expect
  `value_id` for layout queries. Verify against current Flutter stable.

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] All 3 critical issues resolved
- [ ] All 4 major issues resolved or justified with follow-up tickets
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo check --workspace` clean
- [ ] `cargo clippy --workspace -- -D warnings` 0 warnings
- [ ] `cargo test --lib --workspace` all pass
- [ ] Manual test: Enter DevTools mode → Inspector → press `r` → widget tree loads (with connected VM)
- [ ] Manual test: Enter DevTools mode → Inspector → press `r` with no VM → error displayed (not stuck loading)
- [ ] Manual test: Performance panel shows live data when VM connected
- [ ] Manual test: Switch sessions → DevTools state resets
- [ ] Manual test: Press `b` → browser opens DevTools URL
