# Pre-App Custom Sources — Phase 1 Task Index

## Overview

Allow custom sources to start before the Flutter app with configurable readiness checks, gating the Flutter launch until dependencies are healthy.

**Total Tasks:** 8

## Task Dependency Graph

```
Wave 1 (parallel — no deps)
┌──────────────────────────┐     ┌──────────────────────────────┐
│  01-config-types         │     │  02-daemon-stdout-readiness   │
│  ReadyCheck enum +       │     │  ready_pattern + ready_tx in  │
│  CustomSourceConfig ext  │     │  run_custom_capture()         │
└───────────┬──────────────┘     └──────────────┬───────────────┘
            │                                   │
Wave 2 (parallel — depend on wave 1)            │
┌───────────┴──────────────┐     ┌──────────────┴───────────────┐
│  03-message-action-types │     │  04-ready-check-execution     │
│  Message + UpdateAction  │     │  HTTP/TCP/Cmd/Stdout/Delay    │
│  variants                │     │  check runners                │
│  depends: 01             │     │  depends: 01, 02              │
└───────────┬──────────────┘     └──────────────┬───────────────┘
            │                                   │
Wave 3      └───────────┬───────────────────────┘
                        ▼
            ┌───────────────────────────┐
            │  05-launch-flow           │
            │  handle_launch() gating   │
            │  + message handlers       │
            │  depends: 01, 03          │
            └───────────┬───────────────┘
                        │
Wave 4                  ▼
            ┌───────────────────────────┐
            │  06-spawn-pre-app-action  │
            │  spawn_pre_app_sources()  │
            │  orchestration            │
            │  depends: 03, 04, 05      │
            └───────────┬───────────────┘
                        │
Wave 5                  ▼
            ┌───────────────────────────┐
            │  07-double-spawn-guard    │
            │  Skip pre-app sources on  │
            │  AppStarted               │
            │  depends: 06              │
            └───────────┬───────────────┘
                        │
Wave 6                  ▼
            ┌───────────────────────────┐
            │  08-documentation         │
            │  CONFIGURATION.md +       │
            │  ARCHITECTURE.md updates  │
            │  depends: 07              │
            └───────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-config-types](tasks/01-config-types.md) | Not Started | - | `fdemon-app/config/types.rs` |
| 2 | [02-daemon-stdout-readiness](tasks/02-daemon-stdout-readiness.md) | Not Started | - | `fdemon-daemon/native_logs/custom.rs` |
| 3 | [03-message-action-types](tasks/03-message-action-types.md) | Not Started | 1 | `fdemon-app/message.rs`, `fdemon-app/handler/mod.rs` |
| 4 | [04-ready-check-execution](tasks/04-ready-check-execution.md) | Not Started | 1, 2 | `fdemon-app/actions/ready_check.rs` (**NEW**) |
| 5 | [05-launch-flow](tasks/05-launch-flow.md) | Not Started | 1, 3 | `fdemon-app/handler/new_session/launch_context.rs`, `fdemon-app/handler/update.rs` |
| 6 | [06-spawn-pre-app-action](tasks/06-spawn-pre-app-action.md) | Not Started | 3, 4, 5 | `fdemon-app/actions/native_logs.rs`, `fdemon-app/actions/mod.rs` |
| 7 | [07-double-spawn-guard](tasks/07-double-spawn-guard.md) | Not Started | 6 | `fdemon-app/session/handle.rs`, `fdemon-app/actions/native_logs.rs`, `fdemon-app/handler/session.rs` |
| 8 | [08-documentation](tasks/08-documentation.md) | Not Started | 7 | `docs/CONFIGURATION.md`, `docs/ARCHITECTURE.md` |

## Success Criteria

Phase 1 is complete when:

- [ ] `start_before_app = true` causes custom source to spawn before Flutter app launch
- [ ] All five ready check types work (HTTP, TCP, command, stdout, delay)
- [ ] Timeout causes Flutter launch to proceed with warning (not block indefinitely)
- [ ] Pre-app source stdout is visible in log view during readiness wait
- [ ] Progress messages show which sources are pending and readiness timing
- [ ] Hot restart does NOT re-spawn pre-app sources
- [ ] Session close during readiness wait cleans up properly
- [ ] Configs without `start_before_app` are completely unaffected (zero behavioral change)
- [ ] `ready_check` without `start_before_app = true` is a validation error
- [ ] All new code has unit tests
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

## Notes

- Wave 1 tasks (01, 02) can be dispatched in parallel immediately
- Wave 2 tasks (03, 04) can be dispatched in parallel once their deps complete
- Tasks 05-08 are sequential
- The daemon-layer change (02) is isolated to `fdemon-daemon` and can be tested independently
- The ready check module (04) is a new file with no existing code to modify — clean implementation
