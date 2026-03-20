# DAP Server Phase 6: Tier-1 Feature Completion & Variable System Overhaul — Task Index

## Overview

Fix critical bugs in the existing DAP implementation (issue #24 — empty variables panel), implement globals/exception scopes, getter evaluation, `toString()` display, and add missing tier-1 DAP requests (`restartFrame`, `exceptionInfo`, `loadedSources`, `callService`, `updateDebugOptions`, `breakpointLocations`, `completions`). After this phase, fdemon's DAP server fully replaces the built-in debugger for Flutter development.

**Total Tasks:** 18
**Estimated Hours:** 55–80 hours

## Task Dependency Graph

```
Wave 1 (Critical bug fixes + backend expansion — parallel)
┌──────────────────────────────────┐  ┌──────────────────────────────────┐
│  01-fix-variable-display-bugs    │  │  02-expand-backend-trait         │
│  classRef mismatch, source refs  │  │  get_isolate, call_service, etc  │
└────────────────┬─────────────────┘  └────────────────┬─────────────────┘
                 │                                     │
Wave 2 (Variable system — parallel, depend on 01+02)
┌────────────────┴─────────────────┐  ┌────────────────┴─────────────────┐
│  03-globals-scope                │  │  04-exception-scope              │
│  Library static fields           │  │  Exception InstanceRef + scope   │
└────────────────┬─────────────────┘  └────────────────┬─────────────────┘
┌────────────────┴─────────────────┐  ┌────────────────┴─────────────────┐
│  05-variable-type-rendering      │  │  06-evaluate-name-construction   │
│  Record, truncation, map keys    │  │  Watch drill-down support        │
└──────────────────────────────────┘  └──────────────────────────────────┘

Wave 3 (Rich variable display — depend on 03)
┌──────────────────────────────────┐  ┌──────────────────────────────────┐
│  07-getter-evaluation            │  │  08-tostring-display             │
│  Class hierarchy getter eval     │  │  toString() in variable values   │
└──────────────────────────────────┘  └──────────────────────────────────┘

Wave 4 (New DAP requests — parallel, depend on 02)
┌──────────────────────────────────┐  ┌──────────────────────────────────┐
│  09-exception-info-request       │  │  10-restart-frame-request        │
│  Structured exception data       │  │  VM Service kRewind step         │
└──────────────────────────────────┘  └──────────────────────────────────┘
┌──────────────────────────────────┐  ┌──────────────────────────────────┐
│  11-loaded-sources-request       │  │  12-call-service-request         │
│  List all Dart scripts           │  │  Forward VM Service RPCs         │
└──────────────────────────────────┘  └──────────────────────────────────┘

Wave 5 (Advanced features — parallel, depend on 02)
┌──────────────────────────────────┐  ┌──────────────────────────────────┐
│  13-update-debug-options         │  │  14-progress-reporting           │
│  SDK/external library toggle     │  │  Hot reload/restart progress     │
└──────────────────────────────────┘  └──────────────────────────────────┘
┌──────────────────────────────────┐  ┌──────────────────────────────────┐
│  15-breakpoint-locations         │  │  16-completions-request          │
│  Valid BP positions from VM      │  │  Debug console autocomplete      │
└──────────────────────────────────┘  └──────────────────────────────────┘

Wave 6 (Integration + hardening — depend on all above)
┌──────────────────────────────────┐  ┌──────────────────────────────────┐
│  17-request-timeouts-events      │  │  18-documentation-update         │
│  Timeouts + missing DAP events   │  │  Update ARCHITECTURE.md          │
└──────────────────────────────────┘  └──────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-fix-variable-display-bugs](tasks/01-fix-variable-display-bugs.md) | Done | - | 2–3h | `fdemon-dap/adapter/variables.rs`, `fdemon-dap/protocol/types.rs` |
| 2 | [02-expand-backend-trait](tasks/02-expand-backend-trait.md) | Done | - | 3–4h | `fdemon-dap/adapter/backend.rs`, `fdemon-app/handler/dap_backend.rs` |
| 3 | [03-globals-scope](tasks/03-globals-scope.md) | Done | 1, 2 | 3–5h | `fdemon-dap/adapter/variables.rs` |
| 4 | [04-exception-scope](tasks/04-exception-scope.md) | Done | 1, 2 | 3–4h | `fdemon-dap/adapter/variables.rs`, `fdemon-dap/adapter/stack.rs`, `fdemon-dap/adapter/events.rs` |
| 5 | [05-variable-type-rendering](tasks/05-variable-type-rendering.md) | Done | 1 | 3–4h | `fdemon-dap/adapter/variables.rs` |
| 6 | [06-evaluate-name-construction](tasks/06-evaluate-name-construction.md) | Done | 1 | 2–3h | `fdemon-dap/adapter/variables.rs` |
| 7 | [07-getter-evaluation](tasks/07-getter-evaluation.md) | Done | 3 | 4–6h | `fdemon-dap/adapter/variables.rs`, `fdemon-dap/adapter/backend.rs` |
| 8 | [08-tostring-display](tasks/08-tostring-display.md) | Done | 3 | 3–4h | `fdemon-dap/adapter/variables.rs` |
| 9 | [09-exception-info-request](tasks/09-exception-info-request.md) | Done | 2, 4 | 2–3h | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/protocol/types.rs` |
| 10 | [10-restart-frame-request](tasks/10-restart-frame-request.md) | Done | 2 | 3–4h | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/adapter/types.rs`, `fdemon-dap/protocol/types.rs` |
| 11 | [11-loaded-sources-request](tasks/11-loaded-sources-request.md) | Done | 2 | 2–3h | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/protocol/types.rs` |
| 12 | [12-call-service-request](tasks/12-call-service-request.md) | Done | 2 | 2–3h | `fdemon-dap/adapter/handlers.rs` |
| 13 | [13-update-debug-options](tasks/13-update-debug-options.md) | Done | 2 | 4–6h | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/adapter/mod.rs`, `fdemon-dap/adapter/events.rs` |
| 14 | [14-progress-reporting](tasks/14-progress-reporting.md) | Done | 2 | 3–4h | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/adapter/events.rs`, `fdemon-dap/protocol/types.rs` |
| 15 | [15-breakpoint-locations](tasks/15-breakpoint-locations.md) | Done | 2 | 2–3h | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/protocol/types.rs` |
| 16 | [16-completions-request](tasks/16-completions-request.md) | Done | 2 | 3–5h | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/protocol/types.rs` |
| 17 | [17-request-timeouts-events](tasks/17-request-timeouts-events.md) | Done | 1–16 | 4–5h | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/adapter/variables.rs`, `fdemon-dap/adapter/events.rs` |
| 18 | [18-documentation-update](tasks/18-documentation-update.md) | Done | 1–17 | 2–3h | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01-fix-variable-display-bugs | `fdemon-dap/adapter/variables.rs`, `fdemon-dap/protocol/types.rs` | `fdemon-dap/adapter/stack.rs` |
| 02-expand-backend-trait | `fdemon-dap/adapter/backend.rs`, `fdemon-app/handler/dap_backend.rs` | `fdemon-daemon/vm_service/debugger.rs` |
| 03-globals-scope | `fdemon-dap/adapter/variables.rs` | `fdemon-dap/adapter/backend.rs`, `fdemon-dap/adapter/stack.rs` |
| 04-exception-scope | `fdemon-dap/adapter/variables.rs`, `fdemon-dap/adapter/stack.rs`, `fdemon-dap/adapter/events.rs` | `fdemon-dap/adapter/backend.rs` |
| 05-variable-type-rendering | `fdemon-dap/adapter/variables.rs` | `fdemon-dap/adapter/evaluate.rs` |
| 06-evaluate-name-construction | `fdemon-dap/adapter/variables.rs` | - |
| 07-getter-evaluation | `fdemon-dap/adapter/variables.rs`, `fdemon-dap/adapter/backend.rs` | - |
| 08-tostring-display | `fdemon-dap/adapter/variables.rs` | `fdemon-dap/adapter/evaluate.rs` |
| 09-exception-info-request | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/protocol/types.rs` | `fdemon-dap/adapter/events.rs` |
| 10-restart-frame-request | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/adapter/types.rs`, `fdemon-dap/protocol/types.rs` | `fdemon-dap/adapter/stack.rs` |
| 11-loaded-sources-request | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/protocol/types.rs` | `fdemon-dap/adapter/stack.rs` |
| 12-call-service-request | `fdemon-dap/adapter/handlers.rs` | `fdemon-dap/adapter/backend.rs` |
| 13-update-debug-options | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/adapter/mod.rs`, `fdemon-dap/adapter/events.rs` | `fdemon-dap/adapter/backend.rs` |
| 14-progress-reporting | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/adapter/events.rs`, `fdemon-dap/protocol/types.rs` | - |
| 15-breakpoint-locations | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/protocol/types.rs` | `fdemon-dap/adapter/backend.rs` |
| 16-completions-request | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/protocol/types.rs` | `fdemon-dap/adapter/backend.rs`, `fdemon-dap/adapter/stack.rs` |
| 17-request-timeouts-events | `fdemon-dap/adapter/handlers.rs`, `fdemon-dap/adapter/variables.rs`, `fdemon-dap/adapter/events.rs` | - |
| 18-documentation-update | `docs/ARCHITECTURE.md` | All above task files |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | None | Parallel (worktree) |
| 03 + 04 | `variables.rs` | Sequential (same branch) |
| 03 + 05 | `variables.rs` | Sequential (same branch) |
| 03 + 06 | `variables.rs` | Sequential (same branch) |
| 04 + 05 | `variables.rs` | Sequential (same branch) |
| 04 + 06 | `variables.rs` | Sequential (same branch) |
| 05 + 06 | `variables.rs` | Sequential (same branch) |
| 07 + 08 | `variables.rs` | Sequential (same branch) |
| 09 + 10 | `handlers.rs`, `types.rs` | Sequential (same branch) |
| 09 + 11 | `handlers.rs`, `types.rs` | Sequential (same branch) |
| 09 + 12 | `handlers.rs` | Sequential (same branch) |
| 10 + 11 | `handlers.rs`, `types.rs` | Sequential (same branch) |
| 10 + 12 | `handlers.rs` | Sequential (same branch) |
| 11 + 12 | `handlers.rs` | Sequential (same branch) |
| 13 + 14 | `handlers.rs`, `events.rs` | Sequential (same branch) |
| 15 + 16 | `handlers.rs`, `types.rs` | Sequential (same branch) |

**Parallel-safe wave pairs:** Tasks 01+02 (Wave 1). All Wave 4/5 tasks share `handlers.rs` and must be sequential within each wave.

**Recommended execution order within sequential chains:**
- Variables chain (Wave 2–3): 03 → 04 → 05 → 06 → 07 → 08
- Requests chain (Wave 4–5): 09 → 10 → 11 → 12 → 13 → 14 → 15 → 16

## Success Criteria

Phase 6 is complete when:

- [ ] Variables panel shows locals with correct class names (not `"PlainInstance instance"`)
- [ ] Globals scope returns library static fields
- [ ] Exception scope appears when paused at exception
- [ ] `exceptionInfo` request returns structured exception data
- [ ] `restartFrame` rewinds to selected frame; rejects async frames
- [ ] `loadedSources` returns all loaded scripts
- [ ] `callService` forwards arbitrary VM Service RPCs
- [ ] `updateDebugOptions` toggles SDK/external library debugging
- [ ] `breakpointLocations` returns valid breakpoint positions
- [ ] `completions` provides debug console autocomplete
- [ ] Getters evaluated in variables panel (with timeout)
- [ ] `toString()` appended to PlainInstance values
- [ ] Record/WeakReference/Sentinel types render correctly
- [ ] String truncation shows `…` indicator
- [ ] Map complex keys show toString() result
- [ ] `evaluateName` set on all variables
- [ ] Progress events emitted for hot reload/restart
- [ ] All backend calls wrapped with 10s timeout
- [ ] `dart.hotReloadComplete` / `dart.serviceExtensionAdded` events emitted
- [ ] 200+ new unit tests
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` clean

## Notes

- **Wave 2 tasks (03–06) all write to `variables.rs`** — they must execute sequentially despite having no logical dependency on each other. The file overlap matrix forces this.
- **Wave 4–5 tasks (09–16) all write to `handlers.rs`** — same sequential constraint. Recommended order prioritizes critical features first.
- **`fdemon-dap` does NOT depend on `fdemon-app`**: All cross-boundary communication uses the `DebugBackend` trait. New backend methods must be added in both the trait definition (`fdemon-dap`) and the implementation (`fdemon-app/handler/dap_backend.rs`).
- **Mock backend in tests**: The `MockBackend` in test files returns raw JSON directly (not round-tripped through typed structs). This means mock tests won't catch the `"class"` vs `"classRef"` mismatch — Task 01 must fix the production code path, and integration testing is needed to verify.
- **`REQUEST_TIMEOUT` is defined at `adapter/types.rs:232`** with `#[allow(dead_code)]` — Task 17 removes the allow and wires it into all backend calls.
