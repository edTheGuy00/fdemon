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
| 1 | [01-codec-header-line-limit](tasks/01-codec-header-line-limit.md) | Not Started | - | HIGH | `fdemon-dap/protocol/codec.rs` |
| 2 | [02-guard-start-starting-state](tasks/02-guard-start-starting-state.md) | Not Started | - | MEDIUM | `fdemon-app/handler/dap.rs` |
| 3 | [03-remove-unimplemented-capabilities](tasks/03-remove-unimplemented-capabilities.md) | Not Started | - | MEDIUM | `fdemon-dap/protocol/types.rs` |
| 4 | [04-shutdown-timeout-warning](tasks/04-shutdown-timeout-warning.md) | Not Started | - | MEDIUM | `fdemon-dap/service.rs` |
| 5 | [05-consolidate-cli-dap-override](tasks/05-consolidate-cli-dap-override.md) | Not Started | merge | MEDIUM | `fdemon-app/engine.rs`, runners |
| 6 | [06-toggle-transitional-states](tasks/06-toggle-transitional-states.md) | Not Started | merge | MEDIUM | `fdemon-app/handler/dap.rs` |
| 7 | [07-server-hardening](tasks/07-server-hardening.md) | Not Started | merge | LOW | `fdemon-dap/server/mod.rs` |
| 8 | [08-headless-event-cleanup](tasks/08-headless-event-cleanup.md) | Not Started | merge | LOW | `src/headless/`, `fdemon-dap/Cargo.toml` |
| 9 | [09-client-registry](tasks/09-client-registry.md) | Not Started | merge | LOW | `fdemon-app/handler/dap.rs`, `state.rs` |

## Resolved (No Action Needed)

- **Review Item #9 (`support_terminate_debuggee` typo):** Verified correct. The Rust field `support_terminate_debuggee` serializes to `supportTerminateDebuggee` via `#[serde(rename_all = "camelCase")]`, matching the DAP spec exactly. The DAP spec uses singular `support` (not `supports`) for this field.
- **Review Item #13 (verify serde rename):** Same finding — all 10 Capabilities fields serialize to correct DAP spec names.

## Success Criteria

Phase 2 fixes are complete when:

### Pre-Merge (Wave 1)
- [ ] Codec rejects header lines exceeding 4 KB
- [ ] `StartDapServer` is a no-op when `DapStatus::Starting`
- [ ] Initialize response only advertises `supportsConfigurationDoneRequest: true`
- [ ] Shutdown timeout logs a warning and aborts the task
- [ ] `cargo test -p fdemon-dap` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` clean

### Post-Merge (Wave 2)
- [ ] CLI DAP override uses a single `Engine` method
- [ ] `ToggleDap` during `Starting`/`Stopping` is a no-op
- [ ] Accept loop has connection limit (semaphore) and error backoff
- [ ] `DapServerHandle` fields are `pub(crate)` with `port()` accessor
- [ ] DAP port output uses `HeadlessEvent` pattern
- [ ] `fdemon-daemon` dependency removed from `fdemon-dap`
- [ ] Client tracking uses `HashSet<String>` instead of counter
