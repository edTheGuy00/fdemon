# Version Panel Display Fixes - Task Index

## Overview

Fix three display issues in the Flutter Version Panel: tab label disappearing on unfocus, SDK info clipping at small terminal sizes, and incomplete SDK metadata. Phases 1-2 are pure TUI fixes; Phase 3 adds async version probing for complete metadata.

**Total Tasks:** 5

## Task Dependency Graph

```
┌──────────────────────────────┐
│  01-fix-tab-label            │
│  (Bug 3 — always show label) │
└──────────┬───────────────────┘
           │
           ▼
┌──────────────────────────────┐
│  02-fix-vertical-layout      │
│  (Bug 1 — compact mode)     │
└──────────┬───────────────────┘
           │
    ┌──────┴──────────────────────────────┐
    ▼                                     ▼
┌──────────────────────────────┐  ┌──────────────────────────────┐
│  03-version-probe-backend    │  │  04-sdk-info-extended-fields │
│  (fdemon-daemon probe)       │  │  (TUI layout for new fields) │
└──────────┬───────────────────┘  └──────────┬───────────────────┘
           │                                 │
           └──────────┬──────────────────────┘
                      ▼
           ┌──────────────────────────────┐
           │  05-probe-wiring-and-display │
           │  (message/handler/action)    │
           └──────────────────────────────┘
```

### Parallelism Waves

| Wave | Tasks | Can Run In Parallel |
|------|-------|---------------------|
| 1 | 01 | No |
| 2 | 02 | No (depends on 01) |
| 3 | 03, 04 | Yes |
| 4 | 05 | No (depends on 03, 04) |

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-fix-tab-label](tasks/01-fix-tab-label.md) | Done | - | `fdemon-tui: widgets/flutter_version_panel/sdk_info.rs` |
| 2 | [02-fix-vertical-layout](tasks/02-fix-vertical-layout.md) | Done | 01 | `fdemon-tui: widgets/flutter_version_panel/sdk_info.rs, mod.rs` |
| 3 | [03-version-probe-backend](tasks/03-version-probe-backend.md) | Done | - | `fdemon-daemon: flutter_sdk/version_probe.rs (NEW), flutter_sdk/types.rs, flutter_sdk/mod.rs` |
| 4 | [04-sdk-info-extended-fields](tasks/04-sdk-info-extended-fields.md) | Done | 01, 02 | `fdemon-app: flutter_version/state.rs` `fdemon-tui: widgets/flutter_version_panel/sdk_info.rs` |
| 5 | [05-probe-wiring-and-display](tasks/05-probe-wiring-and-display.md) | Done | 03, 04 | `fdemon-app: message.rs, handler/flutter_version/actions.rs, handler/update.rs, actions/mod.rs` `fdemon-tui: widgets/flutter_version_panel/sdk_info.rs` |

## Success Criteria

Version Panel Display Fixes are complete when:

- [ ] "SDK Info" label is always visible with focus-dependent styling (ACCENT when focused, TEXT_SECONDARY when unfocused)
- [ ] SDK info fields are fully visible in both horizontal and vertical layouts at minimum supported terminal size
- [ ] Flutter version shows actual version (e.g., "3.38.6") instead of "unknown" for PATH-inferred SDKs
- [ ] Framework revision, engine hash, and DevTools version are displayed when available
- [ ] Async probe runs non-blocking; failure is graceful with em-dash fallback
- [ ] SOURCE and SDK PATH fields use dynamic column widths (no hardcoded truncation)
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

## Notes

- Tasks 01 and 02 are pure TUI rendering fixes with no data model changes — safe to ship independently.
- Task 03 adds a new async subprocess call (`flutter --version --machine`). This is the first time fdemon-daemon invokes Flutter CLI for metadata collection.
- Task 05 wires everything together and is the integration point. It should be the final task.
- All changes on the existing `feature/flutter-sdk-management` branch.
