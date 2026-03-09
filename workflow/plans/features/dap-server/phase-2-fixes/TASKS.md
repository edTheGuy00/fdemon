# Phase 2 Review Fixes - Task Index

## Overview

Address issues found during the Phase 2 DAP server code review. Pre-merge fixes resolve safety and correctness concerns; post-merge tasks improve robustness and architecture.

**Review Source:** `workflow/reviews/features/dap-server-phase-2/`
**Total Tasks:** 9
**Dispatch Waves:** 2

## Task Dependency Graph

```
Wave 1 — Pre-Merge Fixes (all parallel, no dependencies):
┌───────────────────────┐  ┌───────────────────────┐
│  01-codec-header-     │  │  02-guard-start-      │
│  line-limit           │  │  starting-state       │
│  (fdemon-dap: codec)  │  │  (fdemon-app: handler)│
└───────────────────────┘  └───────────────────────┘

┌───────────────────────┐  ┌───────────────────────┐
│  03-remove-unimp-     │  │  04-shutdown-timeout-  │
│  capabilities         │  │  warning               │
│  (fdemon-dap: types)  │  │  (fdemon-dap: service) │
└───────────────────────┘  └───────────────────────┘

Wave 2 — Post-Merge Improvements (all parallel, after merge):
┌───────────────────────┐  ┌───────────────────────┐
│  05-consolidate-cli-  │  │  06-toggle-transitional│
│  dap-override         │  │  -states              │
│  (fdemon-app: engine) │  │  (fdemon-app: handler) │
└───────────────────────┘  └───────────────────────┘

┌───────────────────────┐  ┌───────────────────────┐
│  07-server-hardening  │  │  08-headless-event-   │
│  (fdemon-dap: server) │  │  cleanup              │
│                       │  │  (binary: headless)   │
└───────────────────────┘  └───────────────────────┘

┌───────────────────────┐
│  09-client-registry   │
│  (fdemon-app: handler)│
└───────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Priority | Modules |
|---|------|--------|------------|----------|---------|
| 1 | [01-codec-header-line-limit](tasks/01-codec-header-line-limit.md) | Done | - | HIGH | `fdemon-dap/protocol/codec.rs` |
| 2 | [02-guard-start-starting-state](tasks/02-guard-start-starting-state.md) | Done | - | MEDIUM | `fdemon-app/handler/dap.rs` |
| 3 | [03-remove-unimplemented-capabilities](tasks/03-remove-unimplemented-capabilities.md) | Done | - | MEDIUM | `fdemon-dap/protocol/types.rs` |
| 4 | [04-shutdown-timeout-warning](tasks/04-shutdown-timeout-warning.md) | Done | - | MEDIUM | `fdemon-dap/service.rs` |
| 5 | [05-consolidate-cli-dap-override](tasks/05-consolidate-cli-dap-override.md) | Done | merge | MEDIUM | `fdemon-app/engine.rs`, runners |
| 6 | [06-toggle-transitional-states](tasks/06-toggle-transitional-states.md) | Done | merge | MEDIUM | `fdemon-app/handler/dap.rs` |
| 7 | [07-server-hardening](tasks/07-server-hardening.md) | Done | merge | LOW | `fdemon-dap/server/mod.rs` |
| 8 | [08-headless-event-cleanup](tasks/08-headless-event-cleanup.md) | Done | merge | LOW | `src/headless/`, `fdemon-dap/Cargo.toml` |
| 9 | [09-client-registry](tasks/09-client-registry.md) | Done | merge | LOW | `fdemon-app/handler/dap.rs`, `state.rs` |

## Resolved (No Action Needed)

- **Review Item #9 (`support_terminate_debuggee` typo):** Verified correct. The Rust field `support_terminate_debuggee` serializes to `supportTerminateDebuggee` via `#[serde(rename_all = "camelCase")]`, matching the DAP spec exactly. The DAP spec uses singular `support` (not `supports`) for this field.
- **Review Item #13 (verify serde rename):** Same finding — all 10 Capabilities fields serialize to correct DAP spec names.

## Success Criteria

Phase 2 fixes are complete when:

### Pre-Merge (Wave 1)
- [x] Codec rejects header lines exceeding 4 KB
- [x] `StartDapServer` is a no-op when `DapStatus::Starting`
- [x] Initialize response only advertises `supportsConfigurationDoneRequest: true`
- [x] Shutdown timeout logs a warning and aborts the task
- [x] `cargo test -p fdemon-dap` passes
- [x] `cargo test --workspace` passes
- [x] `cargo clippy --workspace -- -D warnings` clean

### Post-Merge (Wave 2)
- [x] CLI DAP override uses a single `Engine` method
- [x] `ToggleDap` during `Starting`/`Stopping` is a no-op
- [x] Accept loop has connection limit (semaphore) and error backoff
- [x] `DapServerHandle` fields are `pub(crate)` with `port()` accessor
- [x] DAP port output uses `HeadlessEvent` pattern
- [x] `fdemon-daemon` dependency removed from `fdemon-dap`
- [x] Client tracking uses `HashSet<String>` instead of counter
