# Phase 2: Flutter Version Panel (TUI) - Task Index

## Overview

Build a dedicated TUI panel for viewing and managing Flutter SDK versions. The panel is a centered popup overlay (following the New Session Dialog pattern) opened with `V`, showing the current SDK info and a scrollable list of installed versions from the FVM cache.

**Total Tasks:** 7

## Task Dependency Graph

```
┌───────────────────────────┐     ┌───────────────────────────┐
│  01-state-types           │     │  02-cache-scanner         │
│  (module, types, UiMode)  │     │  (fdemon-daemon)          │
└─────────┬─────────────────┘     └─────────┬─────────────────┘
          │                                 │
   ┌──────┴────────────┐                    │
   │                   │                    │
   ▼                   ▼                    │
┌──────────┐    ┌──────────────┐            │
│    03    │    │     06       │            │
│ messages │    │  TUI widget  │            │
│ + update │    │              │            │
└────┬─────┘    └──────┬───────┘            │
     │                 │                    │
  ┌──┴──────┐          │                    │
  │         │          │                    │
  ▼         ▼          │                    │
┌──────┐ ┌──────┐      │                    │
│  04  │ │  05  │      │                    │
│ hand │ │ keys │      │                    │
│ lers │ │      │      │                    │
└──┬───┘ └──┬───┘      │                    │
   │        │          │                    │
   └────┬───┘──────────┘────────────────────┘
        ▼
   ┌─────────────────┐
   │       07        │
   │  render + wiring│
   └─────────────────┘
```

### Parallelism Waves

| Wave | Tasks | Can Run In Parallel |
|------|-------|---------------------|
| 1 | 01, 02 | Yes |
| 2 | 03, 06 | Yes (03 depends on 01; 06 depends on 01) |
| 3 | 04, 05 | Yes (04 depends on 03; 05 depends on 03) |
| 4 | 07 | No (depends on 02, 04, 05, 06) |

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-state-types](tasks/01-state-types.md) | Done | - | `fdemon-app: flutter_version/mod.rs, state.rs, types.rs` `fdemon-app: state.rs` |
| 2 | [02-cache-scanner](tasks/02-cache-scanner.md) | Done | - | `fdemon-daemon: flutter_sdk/cache_scanner.rs, mod.rs, lib.rs` |
| 3 | [03-messages-and-update](tasks/03-messages-and-update.md) | Done | 01 | `fdemon-app: message.rs, handler/mod.rs, handler/update.rs` |
| 4 | [04-handler-module](tasks/04-handler-module.md) | Done | 03 | `fdemon-app: handler/flutter_version/mod.rs, navigation.rs, actions.rs` |
| 5 | [05-key-routing](tasks/05-key-routing.md) | Done | 03 | `fdemon-app: handler/keys.rs` |
| 6 | [06-tui-widget](tasks/06-tui-widget.md) | Done | 01 | `fdemon-tui: widgets/flutter_version_panel/mod.rs, sdk_info.rs, version_list.rs, mod.rs` |
| 7 | [07-render-integration](tasks/07-render-integration.md) | Done | 02, 04, 05, 06 | `fdemon-tui: render/mod.rs` `fdemon-app: engine.rs, actions/mod.rs` |

## Success Criteria

Phase 2 is complete when:

- [ ] `V` key opens the Flutter Version panel as a centered overlay in Normal mode
- [ ] `Esc` closes the panel and returns to Normal mode
- [ ] Left pane displays: version, channel, source, SDK path, Dart SDK version from the resolved SDK
- [ ] Right pane lists installed versions scanned from `~/fvm/versions/`
- [ ] Active version is highlighted with a marker in the version list
- [ ] `Tab` switches focus between left and right panes
- [ ] `j`/`Down` and `k`/`Up` navigate the version list with proper scrolling
- [ ] `Enter` on a version writes `.fvmrc` and re-resolves the SDK
- [ ] `d` removes a selected non-active version from cache
- [ ] Panel renders correctly in both horizontal and vertical layouts
- [ ] Panel handles edge case: no SDK resolved (shows "No Flutter SDK found")
- [ ] Panel handles edge case: no installed versions in cache (shows empty state)
- [ ] Comprehensive unit tests for state, handlers, cache scanner, and widget rendering
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

## Keyboard Shortcuts

| Key | Mode | Action |
|-----|------|--------|
| `V` | Normal | Open Flutter Version panel |
| `Esc` | FlutterVersion | Close panel |
| `Tab` | FlutterVersion | Switch pane focus |
| `j`/`Down` | FlutterVersion | Navigate down in version list |
| `k`/`Up` | FlutterVersion | Navigate up in version list |
| `Enter` | FlutterVersion | Switch to selected version |
| `d` | FlutterVersion | Remove selected version |
| `u` | FlutterVersion | Update selected version (stub for Phase 3) |
| `i` | FlutterVersion | Install new version (stub for Phase 3) |
| `Ctrl+C` | FlutterVersion | Quit fdemon |

## Notes

- **Follows the New Session Dialog pattern exactly**: own `UiMode`, `centered_rect`, `dim_background`, two-pane layout, pane focus switching, handler decomposition into `navigation.rs` + `actions.rs`.
- **`InstalledSdk` lives in `fdemon-daemon`** alongside the cache scanner, since it describes SDK installation state. Re-exported via `fdemon-daemon::flutter_sdk` for use in `fdemon-app` state.
- **Version switching writes `.fvmrc` only** (minimal `{ "flutter": "<version>" }` JSON). It does not modify FVM's internal state or call the FVM CLI.
- **`i` (install) and `u` (update) are stubs** in Phase 2 — they set a "coming soon" message. Full implementation deferred to Phase 3.
- **Phase 1's `SdkResolved` / `SdkResolutionFailed` messages are reused** for re-resolution after version switching.
- **The cache scanner is async** (filesystem I/O) — triggered via `UpdateAction::ScanInstalledSdks` when the panel opens, results arrive via `Message::FlutterVersionScanCompleted`.
