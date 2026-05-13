## Task: Use `record_fetch_start()` at Auto-Fetch Sites

**Objective**: Replace three direct `inspector.loading = true` assignments with `inspector.record_fetch_start()` so the fetch-start invariant (`loading=true` AND `last_fetch_time=Some(now)`) is enforced centrally. Without this, a spawned task whose terminal message is lost leaves `loading=true` forever — re-introducing the bug this PR was supposed to fix.

**Depends on**: None

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/mod.rs` — three sites at lines 159, 221, 323

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — `record_fetch_start()` method definition (lines 351-354) — confirm signature

### Details

`InspectorState::record_fetch_start()` (`state.rs:351-354`):
```rust
pub fn record_fetch_start(&mut self) {
    self.loading = true;
    self.last_fetch_time = Some(Instant::now());
}
```

Three sites in `handler/devtools/mod.rs` currently set only `loading`:

**Site 1 — line ~159** (inside `handle_enter_devtools_mode`, StartPerformanceMonitoring followup path):
```rust
// Before:
state.devtools_view_state.inspector.loading = true;
Some(crate::message::Message::RequestWidgetTree { session_id })

// After:
state.devtools_view_state.inspector.record_fetch_start();
Some(crate::message::Message::RequestWidgetTree { session_id })
```

Note: this site queues a follow-up `RequestWidgetTree` message which will hit the debounce check (`is_fetch_debounced()` returns `true` while `loading=true`). The reviewer flagged this as a pre-existing debounce-collision concern (`record_fetch_start()` will *also* set `last_fetch_time=Some(now)`, so `is_fetch_debounced()` will still return true on the follow-up). Document this in a code comment — the current path appears intentional (the followup `RequestWidgetTree` is debounce-blocked and the spawned task arrives via the action dispatch elsewhere) but should be verified.

**Site 2 — line ~221** (inside `handle_enter_devtools_mode`, dispatch path):
```rust
// Before:
state.devtools_view_state.inspector.loading = true;
return UpdateResult::action(UpdateAction::FetchWidgetTree { ... });

// After:
state.devtools_view_state.inspector.record_fetch_start();
return UpdateResult::action(UpdateAction::FetchWidgetTree { ... });
```

**Site 3 — line ~323** (inside `handle_switch_panel`, `DevToolsPanel::Inspector` arm):
```rust
// Before:
state.devtools_view_state.inspector.loading = true;
return UpdateResult::action(UpdateAction::FetchWidgetTree { ... });

// After:
state.devtools_view_state.inspector.record_fetch_start();
return UpdateResult::action(UpdateAction::FetchWidgetTree { ... });
```

### Acceptance Criteria

1. All three direct `inspector.loading = true` assignments in `handler/devtools/mod.rs` are replaced with `inspector.record_fetch_start()`.
2. `git grep "inspector.loading = true" crates/fdemon-app/src/handler/devtools/mod.rs` returns no production matches.
3. A unit test asserts that after `handle_enter_devtools_mode` runs (with conditions that hit site 1 or 2), both `inspector.loading` AND `inspector.last_fetch_time` are set.
4. Existing tests covering the same paths continue to pass.

### Testing

```rust
#[test]
fn test_handle_enter_devtools_mode_records_fetch_start_invariant() {
    let mut state = build_state_with_vm_connected_session();
    
    let _ = update(&mut state, Message::EnterDevToolsMode);
    
    let inspector = &state.devtools_view_state.inspector;
    if inspector.loading {
        // If we set loading=true, last_fetch_time must also be set.
        assert!(inspector.last_fetch_time.is_some(),
            "loading=true must be paired with last_fetch_time=Some(...) — \
             use record_fetch_start() instead of direct assignment");
    }
}
```

Additionally consider a generic "no naked loading assignment" lint via grep in `test_lint_*` (some projects have this pattern).

### Notes

- This task is purely a centralization refactor — no behavior change for the happy path.
- The protection it adds: future code-paths that mark `loading=true` will go through the canonical helper and won't introduce divergent invariants.
- If a future task wants a "hung fetch" watchdog (timestamp-based recovery), the invariant established here is what makes the watchdog implementable.
