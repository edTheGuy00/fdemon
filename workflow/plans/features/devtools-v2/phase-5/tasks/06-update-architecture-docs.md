## Task: Update ARCHITECTURE.md with DevTools Subsystem

**Objective**: Add a comprehensive DevTools section to `docs/ARCHITECTURE.md` documenting the panel architecture, VM Service client, handler/widget layering, per-session state structures, and the data flow from VM Service through domain types, state, handler, and widgets.

**Depends on**: 01 (needs final config state)

### Scope

- `docs/ARCHITECTURE.md`: MODIFIED — Add DevTools subsystem section, update Project Structure tree

### Details

#### 1. Update Project Structure tree

In the existing Project Structure section, add the DevTools-related files that are missing:

Under `crates/fdemon-core/src/`:
```
│   │       ├── network.rs         # Network domain types (HttpProfileEntry, NetworkTiming, etc.)
│   │       ├── performance.rs     # Performance domain types (FrameTiming, MemorySample, RingBuffer, etc.)
│   │       ├── widget_tree.rs     # Widget tree types (DiagnosticsNode, LayoutInfo, EdgeInsets)
```

Under `crates/fdemon-daemon/src/`:
```
│   │       ├── vm_service/        # VM Service WebSocket client
│   │       │   ├── mod.rs         # VmServiceHandle, connection management
│   │       │   ├── network.rs     # ext.dart.io.* HTTP/socket profiling
│   │       │   ├── performance.rs # Memory usage, allocation profiling
│   │       │   ├── timeline.rs    # Frame timing from extension stream
│   │       │   └── extensions/    # Inspector, layout, overlays, dumps
```

Under `crates/fdemon-app/src/`:
```
│   │       ├── handler/
│   │       │   ├── devtools/      # DevTools mode handlers
│   │       │   │   ├── mod.rs     # Panel switching, enter/exit, overlays
│   │       │   │   ├── inspector.rs # Widget tree fetch, layout data fetch
│   │       │   │   ├── performance.rs # Frame selection, memory samples, allocations
│   │       │   │   └── network.rs # Network navigation, recording, filter, polling
│   │       ├── session/
│   │       │   ├── network.rs     # NetworkState — per-session network monitoring
│   │       │   └── performance.rs # PerformanceState — per-session perf monitoring
```

Under `crates/fdemon-tui/src/widgets/`:
```
│               ├── devtools/          # DevTools panels
│               │   ├── mod.rs         # Tab bar + panel dispatch
│               │   ├── inspector/     # Widget Inspector (tree + layout explorer)
│               │   │   ├── mod.rs
│               │   │   ├── tree_panel.rs
│               │   │   └── layout_panel.rs
│               │   ├── performance/   # Performance monitoring
│               │   │   ├── mod.rs
│               │   │   ├── styles.rs
│               │   │   ├── frame_chart/  # Frame timing bar chart
│               │   │   └── memory_chart/ # Memory time-series + allocation table
│               │   └── network/       # Network monitor
│               │       ├── mod.rs
│               │       ├── request_table.rs
│               │       └── request_details.rs
```

#### 2. Add DevTools Architecture section

Add a new top-level section after the "Key Patterns" section. Suggested structure:

```markdown
## DevTools Subsystem

The DevTools mode provides three inspection panels — Inspector, Performance, and Network — accessible by pressing `d` when a Flutter session has a VM Service connection.

### Architecture Overview

```
┌──────────────────────────────────────────────────────────┐
│                    DevTools View                          │
│           (fdemon-tui/widgets/devtools/)                  │
│  ┌────────────┐  ┌────────────────┐  ┌────────────────┐  │
│  │ Inspector  │  │  Performance   │  │   Network      │  │
│  │ tree_panel │  │  frame_chart   │  │ request_table  │  │
│  │layout_panel│  │  memory_chart  │  │request_details │  │
│  └──────┬─────┘  └──────┬─────────┘  └──────┬─────────┘  │
└─────────┼───────────────┼───────────────────┼────────────┘
          │               │                   │
          ▼               ▼                   ▼
┌──────────────────────────────────────────────────────────┐
│               DevTools Handlers                          │
│         (fdemon-app/handler/devtools/)                    │
│  inspector.rs   performance.rs   network.rs   mod.rs     │
└─────────┬───────────────┬───────────────────┬────────────┘
          │               │                   │
          ▼               ▼                   ▼
┌──────────────────────────────────────────────────────────┐
│              Per-Session State                            │
│         (fdemon-app/session/)                             │
│  InspectorState    PerformanceState    NetworkState       │
│  (in state.rs)     (performance.rs)    (network.rs)      │
└─────────┬───────────────┬───────────────────┬────────────┘
          │               │                   │
          ▼               ▼                   ▼
┌──────────────────────────────────────────────────────────┐
│              VM Service Client                           │
│        (fdemon-daemon/vm_service/)                        │
│  extensions/    performance.rs    network.rs   timeline   │
└─────────┬───────────────┬───────────────────┬────────────┘
          │               │                   │
          ▼               ▼                   ▼
┌──────────────────────────────────────────────────────────┐
│              Domain Types                                │
│            (fdemon-core/)                                 │
│  widget_tree.rs    performance.rs    network.rs           │
└──────────────────────────────────────────────────────────┘
```

### Panel State Model

DevTools state lives at two levels:

- **View state** (`DevToolsViewState` in `state.rs`): UI-level state shared across sessions — active panel, overlay toggles, VM connection status. Reset when exiting DevTools mode.
- **Session state** (`PerformanceState`, `NetworkState` on `Session`): Per-session data (frame history, memory samples, network entries). Persists across tab switches and survives DevTools mode exit.

### VM Service Data Flow

1. Engine spawns background polling tasks (performance monitor, network monitor) when a session connects
2. Polling tasks call VM Service extensions via `VmServiceHandle`
3. Responses are parsed into domain types (`MemorySample`, `HttpProfileEntry`, etc.)
4. Results sent as `Message` variants to the Engine message channel
5. Handler functions update per-session state
6. TUI renders the updated state on the next frame
```

#### 3. Keep existing content intact

Do NOT modify any existing sections — only add new content and update the Project Structure tree. The existing Engine, TEA, Layer, and Service sections remain unchanged.

### Acceptance Criteria

1. Project Structure tree includes all DevTools-related files across all 4 crates
2. New "DevTools Subsystem" section exists with architecture diagram
3. Panel state model documented (view state vs session state)
4. VM Service data flow documented
5. No existing content removed or altered
6. All file references point to real files in the codebase

### Testing

No code tests — documentation-only task. Verification:

1. Every file path referenced in the new section exists in the repository
2. The architecture diagram accurately reflects the current code structure
3. State flow description matches the actual handler/message/state wiring

### Notes

- **Scope control**: This task adds a focused DevTools section. A comprehensive rewrite of ARCHITECTURE.md is out of scope — only add new content and update the Project Structure tree.
- **Diagram style**: Follow the existing ASCII diagram conventions already used in ARCHITECTURE.md (Unicode box-drawing characters, consistent indentation).
