# Flutter Demon Architecture

This document describes the internal architecture of Flutter Demon, a high-performance TUI for Flutter development written in Rust.

## Table of Contents

- [Overview](#overview)
- [Engine Architecture](#engine-architecture)
- [Design Principles](#design-principles)
- [Project Structure](#project-structure)
- [Module Reference](#module-reference)
- [Key Patterns](#key-patterns)
- [DevTools Subsystem](#devtools-subsystem)
- [DAP Server Subsystem](#dap-server-subsystem)
- [Native Log Capture Subsystem](#native-log-capture-subsystem)
- [Data Flow](#data-flow)
- [Key Types](#key-types)
- [Future Considerations](#future-considerations)

---

## Overview

Flutter Demon is a terminal-based Flutter development environment that manages Flutter processes, provides real-time log viewing, and supports multi-device sessions. The application is built with a layered architecture separating concerns between domain logic, infrastructure, and presentation.

The core of the application is the **Engine** (`app/engine.rs`), which provides shared orchestration for both TUI and headless runners. The Engine encapsulates all state management, message processing, session tracking, and event broadcasting.

```
┌─────────────────────────────────────────────────────────────────┐
│                        Binary (main.rs)                         │
│                   CLI parsing, project discovery                │
└─────────────────────────────────────────────────────────────────┘
                                 │
                   ┌─────────────┴─────────────┐
                   ▼                           ▼
           ┌───────────────┐           ┌───────────────┐
           │  TUI Runner   │           │    Headless   │
           │ (tui/runner)  │           │   (headless)  │
           │ Terminal I/O  │           │  NDJSON out   │
           └───────┬───────┘           └───────┬───────┘
                   │                           │
                   └─────────────┬─────────────┘
                                 ▼
                    ┌─────────────────────────┐
                    │       Engine            │◄──── signal handler
                    │   (app/engine.rs)       │◄──── file watcher
                    │                         │
                    │ • AppState (TEA model)  │
                    │ • Message channel       │
                    │ • Session tasks         │
                    │ • SharedState           │
                    │ • Event broadcast       │
                    └────────┬────────────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
    ┌───────────────┐ ┌──────────┐ ┌──────────────┐
    │  Services     │ │ Daemon   │ │    Core      │
    │ (controllers) │ │(process) │ │ (domain)     │
    └───────────────┘ └──────────┘ └──────────────┘
                             │
                             ▼
                  ┌───────────────────────┐
                  │   Flutter Process     │
                  │   (flutter run)       │
                  └───────────────────────┘
```

---

## Engine Architecture

### Engine (`app/engine.rs`)

The Engine is the shared orchestration core used by both TUI and headless runners. It encapsulates all application state and coordination logic in a single, testable struct.

**Core Responsibilities:**
- **State Management**: Owns the `AppState` (TEA model)
- **Message Channel**: Unified message channel for all events (keyboard, daemon, watcher, signals)
- **Session Task Tracking**: Manages background tasks for each Flutter session
- **Signal Handling**: SIGINT/SIGTERM handling via `shutdown_tx`/`shutdown_rx`
- **File Watcher**: Integrates file watcher with message bridge
- **Shared State**: Provides `SharedState` for service layer consumers
- **Event Broadcasting**: Emits `EngineEvent` to external subscribers (future MCP server)

**Key Methods:**

| Method | Purpose |
|--------|---------|
| `Engine::new(project_path)` | Creates engine with full initialization |
| `process_message(msg)` | Process single message through TEA |
| `drain_pending_messages()` | Process all pending messages |
| `flush_pending_logs()` | Flush batched logs and sync SharedState |
| `flutter_controller()` | Get controller for current session |
| `log_service()` | Get log buffer access |
| `state_service()` | Get app state access |
| `subscribe()` | Subscribe to EngineEvents |
| `shutdown().await` | Stop watcher, cleanup sessions |

**Event Flow:**
```
Input Sources → Message Channel → Engine.process_message() → handler::update()
                                                          ↓
Signal Handler ──────────────────────────────────────────┘
File Watcher   ──────────────────────────────────────────┘
Daemon Tasks   ──────────────────────────────────────────┘
TUI/Headless   ──────────────────────────────────────────┘
                                                          ↓
                                        ┌─────────────────┴─────────────────┐
                                        ▼                                   ▼
                                  handle_action()                  emit_events()
                                  (side effects)                   (EngineEvent)
                                        │                                   │
                                        ▼                                   ▼
                            Spawn session tasks                     Broadcast to
                            Update SharedState                      subscribers
```

### EngineEvent (`app/engine_event.rs`)

Domain events emitted by the Engine after each message processing cycle. This is the primary extension point for pro features.

**Event Categories:**
- **Session Lifecycle**: `SessionCreated`, `SessionStarted`, `SessionStopped`, `SessionRemoved`
- **Phase Changes**: `PhaseChanged` (Initializing → Running → Reloading, etc.)
- **Hot Reload/Restart**: `ReloadStarted`, `ReloadCompleted`, `ReloadFailed`, `RestartStarted`, `RestartCompleted`
- **Logging**: `LogEntry`, `LogBatch` (for high-volume logging)
- **Device Discovery**: `DevicesDiscovered`
- **File Watcher**: `FilesChanged`
- **Engine Lifecycle**: `Shutdown`

### Runner Implementations

Both runners create an Engine and use it as the single source of truth.

**TUI Runner** (`tui/runner.rs`):
- Creates Engine and initializes the terminal
- Runs TUI-specific startup (device selection, Flutter process spawning)
- Main loop: drains pending messages, flushes logs, renders frame, polls for input
- On quit: shuts down Engine, restores terminal

**Headless Runner** (`headless/runner.rs`):
- Creates Engine and spawns a stdin reader for commands
- Auto-starts a Flutter session
- Main loop: receives messages, processes through Engine, emits NDJSON events
- On quit: shuts down Engine

---

## Design Principles

### The Elm Architecture (TEA)

Flutter Demon follows the **TEA pattern** (Model-View-Update) for state management:

1. **Model** (`AppState`) - The complete application state
2. **Messages** (`Message`) - All possible events/actions
3. **Update** (`handler::update`) - Pure function: `(State, Message) → (State, Action)`
4. **View** (`tui::render`) - Renders state to the terminal

This provides:
- Predictable state transitions
- Easy testing (update is pure)
- Clear separation of concerns
- Time-travel debugging potential

### Layered Architecture

The workspace crates enforce clean layer boundaries with **compile-time guarantees**:

| Crate | Responsibility | Dependencies |
|-------|----------------|--------------|
| **flutter-demon (binary)** | CLI, entry point, headless mode | fdemon-core, fdemon-daemon, fdemon-app, fdemon-tui, fdemon-dap |
| **fdemon-tui** | Terminal UI presentation | fdemon-core, fdemon-app |
| **fdemon-app** | State, orchestration, TEA, Engine, services, config, watcher, DAP bridge | fdemon-core, fdemon-daemon, fdemon-dap |
| **fdemon-dap** | DAP protocol types, adapter logic, TCP server, stdio transport | fdemon-core |
| **fdemon-daemon** | Flutter process I/O, device/emulator management | fdemon-core |
| **fdemon-core** | Domain types, events, discovery, error handling | **None** (zero internal deps) |

**Dependency Flow:**
```
fdemon-core (foundation)
    ↓               ↓
fdemon-daemon    fdemon-dap (DAP protocol)
    ↓               ↓
fdemon-app (orchestration + DAP bridge)
    ↓
fdemon-tui (presentation)
    ↓
flutter-demon (binary)
```

### Layer Dependencies Note

The TUI crate depends on App because of the TEA pattern:
- **View** (`tui::render`) must receive **Model** (`AppState`) to render it
- This is the fundamental TEA contract: `View: State → UI`
- The dependency is intentional and necessary, not a violation

**Workspace Benefits:**
- **Compile-time enforcement**: Cargo prevents circular dependencies and violations
- **Independent testing**: Each crate can be tested in isolation
- **Clear boundaries**: Module structure matches crate boundaries
- **Future extensibility**: Crates can be published, reused, or replaced independently
- **Parallel compilation**: Cargo can build independent crates concurrently

### Error Handling

- Custom `Error` enum with domain-specific variants
- `Result<T>` type alias throughout
- Errors are categorized as `fatal` vs `recoverable`
- Rich error context via `ResultExt` trait

---

## Project Structure

Flutter Demon is organized as a **Cargo workspace** with 5 library crates and 1 binary:

```
flutter-demon/
├── Cargo.toml                    # Workspace root + binary configuration
├── src/
│   ├── main.rs                   # Binary entry point, CLI handling
│   └── headless/                 # Headless NDJSON mode
│       ├── mod.rs                # HeadlessEvent types
│       └── runner.rs             # Headless runner (uses Engine)
│
├── crates/
│   ├── fdemon-core/              # Domain types (zero internal deps)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs          # LogEntry, LogLevel, AppPhase
│   │       ├── events.rs         # DaemonMessage, DaemonEvent + 9 event structs
│   │       ├── discovery.rs      # Flutter project detection
│   │       ├── stack_trace.rs    # Stack trace parsing
│   │       ├── ansi.rs           # ANSI escape sequence handling
│   │       ├── error.rs          # Error types and Result alias
│   │       ├── logging.rs        # File-based logging setup
│   │       ├── prelude.rs        # Common imports
│   │       ├── network.rs        # Network domain types (HttpProfileEntry, NetworkTiming, etc.)
│   │       ├── performance.rs    # Performance domain types (FrameTiming, MemorySample, RingBuffer, etc.)
│   │       ├── frame_hints.rs    # Refresh-rate-aware frame analysis hints (Phase 2 helper)
│   │       ├── rebuild_stats.rs  # Widget rebuild telemetry types (Location, LocationMap, RebuildLocation, RebuildStatsSnapshot, RebuildEventPayload, parse_rebuilt_widgets_event)
│   │       ├── timeline.rs       # VM timeline event types (TimelineThread, TimelinePhase, TimelineEvent, TimelineNode, TimelineTrack, ThreadMetadata, pair_be_events, build_tracks, parse_vm_timeline, parse_vm_timeline_with_metadata)
│   │       └── widget_tree.rs    # Widget tree types (DiagnosticsNode, LayoutInfo, EdgeInsets, FlexChild, FlexFit, Axis, MainAxisAlignment, CrossAxisAlignment, MainAxisSize)
│   │
│   ├── fdemon-daemon/            # Flutter process management
│   │   ├── Cargo.toml            # depends: fdemon-core
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── process.rs        # FlutterProcess spawning/lifecycle
│   │       ├── protocol.rs       # parse_daemon_message() and conversion functions
│   │       ├── commands.rs       # Command sending with request tracking
│   │       ├── devices.rs        # Device discovery
│   │       ├── emulators.rs      # Emulator discovery and launch
│   │       ├── avds.rs           # Android AVD utilities
│   │       ├── simulators.rs     # iOS simulator utilities
│   │       ├── tool_availability.rs  # Tool detection (adb, xcrun simctl, idevicesyslog)
│   │       ├── test_utils.rs     # Test helpers
│   │       ├── flutter_sdk/      # Flutter SDK detection and executable abstraction
│   │       │   ├── mod.rs        # Public API: find_flutter_sdk(), FlutterSdk
│   │       │   ├── locator.rs    # 12-strategy locator (env vars, PATH via which, version managers, shim fallback)
│   │       │   ├── types.rs      # FlutterExecutable enum (Direct, WindowsBatch) + validate_sdk_path
│   │       │   ├── diagnostics.rs  # Shared diagnostic helpers: windows_hint(), is_path_resolution_error(), strip_ansi()
│   │       │   ├── version_probe.rs  # flutter --version parsing
│   │       │   ├── cache_scanner.rs  # Version-manager cache directory scanning
│   │       │   ├── channel.rs    # Flutter channel detection
│   │       │   ├── version_managers.rs  # fvm, asdf, mise strategy helpers
│   │       │   └── windows_tests.rs  # Windows-only integration tests
│   │       ├── native_logs/      # Native platform log capture
│   │       │   ├── mod.rs        # NativeLogCapture trait, shared types, platform dispatch
│   │       │   ├── android.rs    # adb logcat capture
│   │       │   ├── macos.rs      # macOS log stream capture
│   │       │   └── ios.rs        # iOS simulator (xcrun simctl) + physical (idevicesyslog)
│   │       ├── toolchain/        # Toolchain diagnostics (Phase 1) + install (Phase 2 + 3)
│   │       │   ├── mod.rs        # run_preflight() — orchestration entry point; re-exports Phase 2/3 install API + resolve_android_sdk_root_path
│   │       │   ├── types.rs      # Phase 1: ToolchainReport, ComponentCheck, etc. Phase 2: InstallMethod, HostArch, FlutterRelease, FlutterInstallTarget, DownloadProgress, FlutterInstallOutcome. Phase 3: AndroidInstallTarget, AndroidInstallOutcome, cmdline_tools_url, sdkmanager_packages, DEFAULT_CMDLINE_TOOLS_BUILD
│   │       │   ├── checks/       # Per-component probes (mod.rs, android.rs, prerequisites.rs) — no install or network code; android.rs owns resolve_android_sdk_root_path (shared SDK-root resolver) and android_sdk_root() (is_dir()-filtered wrapper)
│   │       │   ├── doctor.rs     # flutter doctor -v capture + marker parser
│   │       │   ├── download.rs   # Streaming archive download, SHA-256 verify, zip + tar.xz extraction (pure-Rust lzma-rs)
│   │       │   ├── process_stream.rs  # run_streaming — merged stdout/stderr line streaming; run_streaming_with_input — stdin-fed variant for non-interactive license acceptance (Phase 3)
│   │       │   ├── flutter_install.rs # Managed Flutter SDK install: git clone (default) or archive download fallback
│   │       │   ├── android_install.rs # Managed Android SDK install: cmdline-tools download, cmdline-tools/latest relocation, sdkmanager license acceptance, sdkmanager package install; streams InstallEvent (Phase 3)
│   │       │   ├── jdk.rs             # resolve_jdk_home (JAVA_HOME / which-walk), configure_flutter_jdk_dir (flutter config --jdk-dir) (Phase 3)
│   │       │   └── path_config.rs     # Idempotent marker-fenced PATH export writes to bash/zsh/fish rc files; add_android_env writes ANDROID_HOME + cmdline-tools/platform-tools PATH entries in a distinct fence block (Phase 3)
│   │       └── vm_service/       # VM Service WebSocket client
│   │           ├── mod.rs        # VmServiceHandle, VmRequestHandle, connection management
│   │           ├── client.rs     # WebSocket client transport
│   │           ├── protocol.rs   # JSON-RPC protocol types
│   │           ├── errors.rs     # VM Service error types
│   │           ├── logging.rs    # VM Service logging utilities
│   │           ├── network.rs    # ext.dart.io.* HTTP/socket profiling
│   │           ├── performance.rs # Memory usage, allocation profiling
│   │           ├── timeline.rs   # Frame timing from extension stream; Phase 3 adds get_vm_timeline_micros, fetch_timeline_chunk; Phase 4 adds fetch_timeline_chunk_with_metadata
│   │           └── extensions/   # Inspector, layout, overlays, dumps
│   │               ├── mod.rs
│   │               ├── inspector.rs
│   │               ├── layout.rs
│   │               ├── properties.rs  # getProperties response parsing + widget/render-object split
│   │               ├── overlays.rs
│   │               ├── dumps.rs
│   │               └── performance.rs  # set_profile_widget_builds, get_profile_widget_builds; PROFILE_WIDGET_BUILDS and WIDGET_LOCATION_ID_MAP ext constants
│   │
│   ├── fdemon-app/               # Application state and orchestration
│   │   ├── Cargo.toml            # depends: fdemon-core, fdemon-daemon
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs         # Engine - shared orchestration core
│   │       ├── engine_event.rs   # EngineEvent - domain events
│   │       ├── state.rs          # AppState (the Model)
│   │       ├── message.rs        # Message enum (all events)
│   │       ├── signals.rs        # SIGINT/SIGTERM handling
│   │       ├── handler/          # TEA update function + helpers
│   │       │   └── devtools/     # DevTools mode handlers
│   │       │       ├── mod.rs    # Panel switching, enter/exit, overlays
│   │       │       ├── inspector.rs  # Widget tree fetch, layout data fetch
│   │       │       ├── performance/  # Performance handlers (split from performance.rs in Phase 2)
│   │       │       │   ├── mod.rs         # Re-exports; panel entry/exit
│   │       │       │   ├── frame.rs       # Frame chart navigation and selection
│   │       │       │   ├── details.rs     # Details pane tab cycling and section focus
│   │       │       │   ├── rebuild_stats.rs # Rebuild stats toggle, event ingestion, table navigation
│   │       │       │   └── timeline.rs    # Timeline event ingestion, filter cycle, list navigation
│   │       │       ├── memory.rs     # Memory samples, allocation profile, memory chart/table nav
│   │       │       └── network.rs    # Network navigation, recording, filter, polling
│   │       ├── session/          # Per-device session state
│   │       │   ├── mod.rs
│   │       │   ├── session.rs    # Session struct and core state
│   │       │   ├── handle.rs     # SessionHandle
│   │       │   ├── network.rs    # NetworkState — per-session network monitoring
│   │       │   ├── performance.rs # PerformanceState — per-session perf monitoring
│   │       │   ├── memory.rs      # MemoryState — per-session memory monitoring
│   │       │   └── native_tags.rs # NativeTagState — per-session tag discovery/filtering
│   │       ├── session_manager.rs  # Multi-session coordination
│   │       ├── watcher.rs        # File system watching
│   │       ├── config/           # Configuration parsing
│   │       │   ├── types.rs      # LaunchConfig, Settings types
│   │       │   ├── settings.rs   # .fdemon/config.toml loader
│   │       │   ├── launch.rs     # .fdemon/launch.toml loader
│   │       │   └── vscode.rs     # .vscode/launch.json compatibility
│   │       ├── services/         # Reusable service layer
│   │       │   ├── flutter_controller.rs  # Reload/restart operations
│   │       │   ├── log_service.rs         # Log buffer access
│   │       │   └── state_service.rs       # Shared state management
│   │       ├── editor.rs         # Editor integration
│   │       ├── settings_items.rs # Setting item generators
│   │       ├── log_view_state.rs # Scroll/viewport state
│   │       ├── hyperlinks.rs     # Link detection and state
│   │       ├── confirm_dialog.rs # Dialog state
│   │       ├── install_wizard/   # Toolchain diagnostics modal state
│   │       │   ├── mod.rs        # Public re-exports
│   │       │   ├── state.rs      # InstallWizardState, WizardStep, build_steps(), installed_sdk_path, selected_guided_command(); Phase 3 adds GuidedCommand population for AndroidTools (JDK gate); Phase 4 adds selected_command_index field, select_next_command()/select_prev_command(), index-aware selected_guided_command(), and prerequisites_guided_commands() (pure function of ToolchainReport — no which::which I/O)
│   │       │   └── types.rs      # WizardStepKind, StepStatus, WizardPane, StepExecution, StepExecStatus (Phase 2); GuidedCommand (Phase 3). Phase 5 followup adds StepExecStatus::Cancelled — terminal, neutral rendering, distinct from Failed, retriable.
│   │       ├── handler/install_wizard/  # Navigation + action handlers for InstallWizard
│   │       │   ├── mod.rs        # Re-export shim (public API surface for the sub-module)
│   │       │   ├── navigation.rs # Navigation handlers (up/down, pane switch); Phase 4 adds handle_prev_command/handle_next_command for [ / ] guided-command index cycling
│   │       │   └── actions.rs    # WizardStep* message handlers + completion chain (Phase 2); Phase 3: AndroidTools step gated on JDK Ok, PathConfig step now also writes ANDROID_HOME, InstallWizardCopyCommand handler copies selected_guided_command() to clipboard; Phase 4: Prerequisites Enter arm split from Doctor — emits guided "Run the listed command(s), then press r to re-check." message instead of the "later phase" stub
│   │       └── new_session_dialog/  # New session dialog state
│   │           ├── state.rs
│   │           ├── fuzzy.rs
│   │           ├── target_selector_state.rs
│   │           └── device_groups.rs
│   │
│   └── fdemon-tui/               # Terminal UI (Ratatui)
│       ├── Cargo.toml            # depends: fdemon-core, fdemon-app
│       └── src/
│           ├── lib.rs
│           ├── runner.rs         # TUI runner (creates Engine)
│           ├── startup.rs        # TUI-specific startup
│           ├── render/           # State → UI rendering
│           │   ├── mod.rs
│           │   └── tests.rs
│           ├── layout.rs         # Layout calculations
│           ├── event.rs          # Terminal event handling
│           ├── terminal.rs       # Terminal setup/restore
│           ├── selector.rs       # Project selection UI
│           ├── test_utils.rs     # TestTerminal wrapper
│           └── widgets/          # Reusable UI components
│               ├── header.rs
│               ├── tabs.rs
│               ├── log_view/     # Scrollable log display
│               │   ├── mod.rs
│               │   ├── styles.rs
│               │   └── tests.rs
│               ├── status_bar.rs
│               ├── device_selector.rs
│               ├── settings_panel/
│               │   ├── mod.rs
│               │   └── styles.rs
│               ├── confirm_dialog.rs
│               ├── tag_filter.rs     # Native tag filter overlay (toggle visibility per tag)
│               ├── new_session_dialog/
│               │   ├── mod.rs
│               │   └── target_selector.rs
│               ├── install_wizard/   # Toolchain diagnostics panel
│               │   ├── mod.rs        # Panel entry point; Phase 6: panel uses 85% height / 28% left pane / 2-row header (MIN_RENDER_HEIGHT == 12)
│               │   ├── step_list.rs  # Left pane: ordered step list
│               │   ├── step_detail.rs # Right pane: per-step detail view + Enter action hints; Phase 3 adds guided-command block (label, command, optional note) with c-key copy hint; Phase 6: component rows and doctor lines wrap via Paragraph::Wrap { trim: false } + local wrapped_height helper (per-item height advance) instead of clipping into 1-row rects; guided-command windowing (command_block_height / guided_section_full_height / compute_guided_window) threads width and accounts for wrapped row counts
│               │   ├── progress.rs    # StepProgress widget — live progress bar, byte counter, log tail (Phase 2)
│               │   └── doctor_view.rs # Embedded flutter doctor -v output view; Phase 6: each DoctorLine wraps via Paragraph::Wrap { trim: false } + local wrapped_height helper so long lines are visible rather than clipped
│               └── devtools/         # DevTools panels
│                   ├── mod.rs        # Tab bar + panel dispatch
│                   ├── inspector/    # Widget Inspector (tree + layout explorer)
│                   │   ├── mod.rs
│                   │   ├── tree_panel.rs
│                   │   ├── layout_panel.rs
│                   │   └── details/  # Inspector details view (Phase 2+)
│                   │       ├── mod.rs
│                   │       ├── properties_tab.rs
│                   │       ├── render_object_tab.rs
│                   │       └── flex_explorer_tab.rs
│                   ├── performance/  # Performance monitoring (dual-pane: chart + details)
│                   │   ├── mod.rs    # PerformancePanel; dual-pane layout + responsive thresholds
│                   │   ├── styles.rs
│                   │   ├── tests.rs
│                   │   ├── frame_chart/  # Frame timing bar chart
│                   │   │   ├── mod.rs
│                   │   │   ├── bars.rs
│                   │   │   └── detail.rs
│                   │   └── details/  # Details pane (Phase 2+)
│                   │       ├── mod.rs               # DetailsPane dispatcher + tab bar
│                   │       ├── frame_analysis_tab.rs # Frame Analysis tab (populated in Phase 2)
│                   │       ├── rebuild_stats_tab.rs  # Rebuild Stats tab (widget rebuild counts and locations; conditionally visible when rebuild tracking is enabled)
│                   │       ├── text_helpers.rs       # pub(super) helpers: truncate_with_ellipsis, pad_right, pad_left, PLACEHOLDER_LINE_COUNT
│                   │       └── timeline_events/      # Timeline Events tab — Gantt chart (Phase 4)
│                   │           ├── mod.rs            # Entry point: filter strip + render dispatch
│                   │           ├── gantt.rs          # Thread rows, depth-stacked bars, time axis
│                   │           ├── palette.rs        # Two-color depth-alternating palette per TimelineThread
│                   │           └── viewport.rs       # Pure math: compute_viewport, micros_to_column, clip_bar
│                   ├── memory/       # Memory monitoring (MemoryPanel widget)
│                   │   ├── mod.rs    # MemoryPanel — top-level memory widget
│                   │   ├── chart.rs  # Memory time-series chart
│                   │   ├── table.rs  # Class allocation table
│                   │   ├── braille_canvas.rs  # Braille-pixel rendering
│                   │   └── tests.rs
│                   └── network/      # Network monitor
│                       ├── mod.rs
│                       ├── request_table.rs
│                       └── request_details.rs
│
├── crates/fdemon-dap/            # DAP server (protocol + adapter + transport)
│   ├── Cargo.toml                # depends: fdemon-core (no daemon/app deps)
│   └── src/
│       ├── lib.rs
│       ├── protocol/             # DAP wire protocol
│       │   ├── mod.rs
│       │   ├── types.rs          # All DAP request/response/event types (incl. Phase 6 types)
│       │   └── codec.rs          # Content-Length framing encode/decode
│       ├── adapter/              # DAP ↔ VM Service translation
│       │   ├── mod.rs            # DapAdapter struct, ExceptionRef, re-exports
│       │   ├── backend.rs        # DebugBackend / LocalDebugBackend trait, DynDebugBackend, BackendError
│       │   ├── handlers.rs       # handle_request dispatch + all per-command handlers
│       │   ├── breakpoints.rs    # BreakpointState, conditions, logpoints
│       │   ├── variables.rs      # Variable expansion, type rendering, getter eval, toString enrichment
│       │   ├── evaluate.rs       # Expression evaluation, EvalContext
│       │   ├── events.rs         # Event emission helpers (progress, custom events)
│       │   ├── stack.rs          # FrameStore, VariableStore (MAX_VARIABLE_REFS), SourceReferenceStore
│       │   ├── threads.rs        # ThreadMap, MultiSessionThreadMap, ID namespacing
│       │   └── types.rs          # StepMode (incl. Rewind), DapExceptionPauseMode, DebugEvent, REQUEST_TIMEOUT
│       ├── server/               # TCP listener + session lifecycle
│       │   ├── mod.rs            # DapServer, TCP accept loop
│       │   └── session.rs        # DapClientSession, NoopBackend (test helper)
│       └── transport/            # Stdio transport
│           ├── mod.rs
│           └── stdio.rs          # Stdio DAP transport for IDE integration testing
│
└── tests/                        # Integration tests (binary crate)
    ├── common/
    └── e2e/
```

---

## Module Reference

### `fdemon-core` — Domain Types (Foundation Crate)

**Location**: `crates/fdemon-core/`
**Dependencies**: Zero internal dependencies (only external crates)
**Purpose**: Pure business logic types with no infrastructure dependencies

| File | Purpose |
|------|---------|
| `types.rs` | `AppPhase`, `LogEntry`, `LogLevel`, `LogSource` — core domain types. `AppPhase` variants: `Initializing` (default), `Preparing` (pre-app native-log sources polling, Flutter not yet spawned), `Launching` (process attached, building/first-run), `Running`, `Reloading`, `Stopped`, `Quitting`. |
| `events.rs` | `DaemonMessage`, `DaemonEvent`, and all 9 event structs (`AppStart`, `AppLog`, `DeviceInfo`, etc.) — events from the Flutter process |
| `discovery.rs` | Flutter project detection: `is_runnable_flutter_project()`, `discover_flutter_projects()`, `ProjectType` enum |
| `stack_trace.rs` | Stack trace parsing and rendering |
| `ansi.rs` | ANSI escape sequence handling |
| `error.rs` | Custom `Error` enum with variants for each error category. Phase 5 adds `Error::Cancelled` (recoverable) — returned when a `CancellationToken` fires during a download or install. Includes `Result<T>` alias and `ResultExt` trait for error context |
| `logging.rs` | Sets up file-based logging via `tracing` (stdout is owned by TUI) |
| `prelude.rs` | Re-exports common types (`Result`, `Error`, tracing macros) |

### `fdemon-daemon` — Flutter Process Infrastructure

**Location**: `crates/fdemon-daemon/`
**Dependencies**: `fdemon-core`; Phase 2 toolchain install adds: `reqwest` (rustls-tls, archive download), `zip`, `tar`, `lzma-rs` (tar.xz extraction), `sha2` (SHA-256 verification). Phase 5 adds: `fs4` (disk-space preflight in `toolchain/download.rs`), `tokio-util` (provides `CancellationToken` for abortable downloads). Phase 7 promotes `tempfile` from `[dev-dependencies]` to `[dependencies]` (used by `path_config.rs` for atomic rc-file writes). All network and archive code is isolated inside `toolchain/`.
**Purpose**: Manages Flutter child processes, JSON-RPC communication, and toolchain installation

| File | Purpose |
|------|---------|
| `process.rs` | `FlutterProcess` — spawns `flutter run --machine`, manages stdin/stdout/stderr streams |
| `protocol.rs` | `parse_daemon_message()`, `to_log_entry()`, `parse_flutter_log()`, `detect_log_level()` — converts JSON-RPC to typed events (event types in `fdemon-core`) |
| `commands.rs` | `CommandSender`, `DaemonCommand`, `RequestTracker` — send commands with request ID tracking |
| `devices.rs` | `Device` type, `discover_devices()` — finds connected devices |
| `emulators.rs` | `Emulator` type, `discover_emulators()`, `launch_emulator()` |
| `avds.rs` | Android AVD utilities |
| `simulators.rs` | iOS simulator utilities |
| `tool_availability.rs` | Tool detection (`adb`, `xcrun simctl`, `idevicesyslog`, `log`). `IosLogTool` enum selects the iOS capture backend at runtime. |
| `test_utils.rs` | Test helpers for device/emulator testing |
| `flutter_sdk/mod.rs` | Public API: `find_flutter_sdk()`, `FlutterSdk` — entry point for SDK detection |
| `flutter_sdk/locator.rs` | 12-strategy locator: explicit config, env vars, version managers (`fvm`, `asdf`, `mise`), system PATH, binary-only shim fallback. Strategy 10 uses `which::which("flutter")` for PATHEXT-aware discovery on Windows. When the legacy `VERSION` file is blank, the locator falls back to `flutter.version.json` for the SDK version string; the manifest channel is reported for git-less installs via `read_channel_from_version_json`. See source for full strategy list. |
| `flutter_sdk/types.rs` | `FlutterExecutable` enum and `validate_sdk_path` / `validate_sdk_path_lenient` |
| `flutter_sdk/diagnostics.rs` | Shared diagnostic helpers used by `devices.rs` and `emulators.rs` — `windows_hint()` (Windows-only, hints at `[flutter] sdk_path`), `is_path_resolution_error()` (stderr predicate to gate the hint), `strip_ansi()` (cleans Flutter CLI color codes from stderr before user-facing display; peeks before consuming the character following an inner ESC in OSC sequences, so a malformed string terminator never drops the next real character). |
| `flutter_sdk/version_probe.rs` | Parses `flutter --version` output |
| `flutter_sdk/cache_scanner.rs` | Scans version-manager cache directories |
| `flutter_sdk/channel.rs` | Flutter channel detection. `read_channel_from_version_json` parses the `flutter.version.json` manifest (present in git-less installs); used as the fallback channel source when the legacy `VERSION` file is blank or absent. |
| `flutter_sdk/version_managers.rs` | fvm / asdf / mise strategy helpers |
| `native_logs/mod.rs` | `NativeLogCapture` trait, `NativeLogHandle`, shared types (`NativeLogEvent`, `AndroidLogConfig`, `MacOsLogConfig`, `IosLogConfig`), and `create_native_log_capture()` platform dispatch |
| `native_logs/android.rs` | `AndroidLogCapture` — spawns `adb logcat`, parses logcat output |
| `native_logs/macos.rs` | `MacOsLogCapture` — spawns `log stream`, parses macOS unified log output |
| `native_logs/ios.rs` | `IosLogCapture` — simulator via `xcrun simctl log stream`, physical via `idevicesyslog` (macOS-only, `#[cfg(target_os = "macos")]`) |
| `native_logs/custom.rs` | `CustomLogCapture` — spawns user-defined commands, reads stdout through format parsers; `CustomSourceConfig` — config for a single custom source; `create_custom_log_capture()` factory |
| `native_logs/formats.rs` | `parse_line()` dispatch — routes raw output lines to `parse_raw()`, `parse_json()`, `parse_logcat_threadtime()`, or `parse_syslog()` based on `OutputFormat` |
| `toolchain/mod.rs` | `run_preflight(project_path, explicit_sdk_path) -> ToolchainReport` — orchestrates all component checks and doctor text capture; never returns `Err`. Reuses `find_flutter_sdk` + `probe_flutter_version`. Re-exports all public Phase 2 and Phase 3 install symbols (including `install_android_tools`, `resolve_jdk_home`, `configure_flutter_jdk_dir`, `add_android_env`, `AndroidInstallTarget`, `AndroidInstallOutcome`) and the shared SDK-root resolver `resolve_android_sdk_root_path`. |
| `toolchain/types.rs` | Phase 1 report types: `ToolchainReport`, `ComponentCheck`, `ComponentStatus`, `ComponentKind`, `HostPlatform`, `HostShell`, `DoctorLine`, `DoctorMarker`. Phase 2 install types: `InstallMethod`, `HostArch`, `FlutterRelease`, `FlutterReleaseManifest`, `FlutterInstallTarget`, `DownloadProgress`, `FlutterInstallOutcome`. Phase 3 Android install types: `AndroidInstallTarget` (sdk_root, api_level, cmdline_tools_build, jdk_path), `AndroidInstallOutcome` (sdk_root, installed_packages), `cmdline_tools_url` (URL builder by platform/build), `sdkmanager_packages` (required package list for an API level), `DEFAULT_CMDLINE_TOOLS_BUILD`. Phase 4 followup: `ToolchainReport` gains two pre-computed environment-detection fields — `linux_package_manager: Option<LinuxPackageManager>` (populated on Linux by `run_preflight`; `None` on other platforms) and `winget_available: bool` (populated on Windows by `run_preflight`; always `false` elsewhere) — so that `prerequisites_guided_commands` in `fdemon-app` is a pure function of the report with no synchronous `which::which` I/O inside the TEA `update()` path. |
| `toolchain/checks/` | Structured per-component probes — Flutter SDK, git, JDK, Android cmdline-tools/sdkmanager, platform-tools/adb, Android platforms, build-tools, licenses, and per-OS prerequisites (`mod.rs`, `android.rs`, `prerequisites.rs`). No install or network code. `android.rs` owns `resolve_android_sdk_root_path(override_path: Option<&Path>) -> PathBuf` — the single shared SDK-root resolver (env-var chain + platform default; always returns a `PathBuf` even when the path does not yet exist). `android_sdk_root() -> Option<AndroidSdkRoot>` is a thin check-time wrapper that delegates to `resolve_android_sdk_root_path(None)` and then filters with `is_dir()`. Both are re-exported via `checks/mod.rs` → `toolchain/mod.rs` → `fdemon-daemon` lib, making `resolve_android_sdk_root_path` the single source of truth consumed by both the install executor and post-install checks. Phase 4 followup: `prerequisites.rs` gains the pure helper `detect_from_candidates(present: &[&str]) -> LinuxPackageManager` (precedence-ordered mapping from which-found binaries to package manager enum); `detect_linux_package_manager` dispatches through it, enabling deterministic unit tests without filesystem probing. Phase 6: `check_linux_prerequisites` also probes GLU dev-headers (key `PREREQ_KEY_GLU = "libglu1-mesa"`) and libstdc++ (compiler-presence heuristic: treated as present when `clang` or `g++` is on PATH, key `PREREQ_KEY_LIBSTDCPP = "libstdc++"`); both are `Partial` (header-only absent) when all required binaries are present, or `Missing` when aggregated with absent binaries. Phase 7: GLU and GTK header probing uses the resolved pkg-config binary (`pkgconf` or `pkg-config`, whichever was detected first) rather than always invoking `pkg-config` by name, so Linux systems that have only `pkgconf` installed get correct results. Rosetta detection on macOS uses filesystem and `pkgutil` checks rather than probing for the `oahd` process, so Rosetta that is installed but idle (no active translation) is correctly reported as present rather than missing. The stale `xz-utils` package alias was removed from the prerequisites list. |
| `toolchain/doctor.rs` | `flutter doctor -v` text capture and marker parser; recognises `[✓]`, `[!]`, `[✗]`, `[☠]` markers to produce `Vec<DoctorLine>`. Stderr deduplication uses exact `trim()` equality (not substring `contains`) to avoid incorrectly suppressing distinct lines that share a common prefix. |
| `toolchain/download.rs` | Streaming archive download (`download_to_file` with `DownloadProgress` callback, download timeout, bounded retry, `.part`-file-rename on completion), SHA-256 verification (`verify_sha256`), traversal-safe zip extraction (`extract_zip` — zip-slip path sanitization, symlink guards), traversal-safe tar.xz extraction (`extract_tar_xz` — tar path + symlink guards, fail-closed on path-traversal and symlink-escape attempts, streaming xz decode via mpsc channel for bounded RAM usage, pure-Rust `lzma-rs`), and unified dispatch (`extract_archive`). Phase 7 security hardening: `download_to_file` enforces HTTPS-only and a bounded 5-hop redirect policy that rejects scheme-downgrade redirects (HTTP ← HTTPS). `AndroidInstallTarget` carries an optional `cmdline_tools_sha256: Option<String>`; when present the Android cmdline-tools archive is verified against this checksum before extraction. Phase 5 adds: `ensure_disk_space(path, required_bytes)` (uses `fs4` statvfs — called before download and extraction to fail fast with a clear error rather than running out of space mid-transfer); `check_network_connectivity(url)` (≤5 s HEAD probe — called in `fetch_release_manifest` before initiating an archive download; documented limitation: cannot distinguish a captive portal from the real host over HTTPS); `CancellationToken` (from `tokio-util`) threaded through `download_to_file`, `install_flutter`, and `install_android_tools` via `tokio::select!`; cancellation is also checked during SHA-256 verification and extraction (`extract_archive`/`extract_zip`/`extract_tar_xz`), polled every 256 entries; `PartFileGuard` RAII type that removes the `.part` file on `Drop` so aborted downloads leave no partial artefacts. Phase 5 followup: `IDLE_TIMEOUT` is wired as `.read_timeout()` (per-read idle guard — resets after each successful chunk read) rather than a total-request deadline, so large SDK downloads over slower connections are not falsely aborted; `TempDirGuard` RAII struct in both `flutter_install.rs` and `android_install.rs` removes the extraction temp dir on `Drop` and is **disarmed only after a successful atomic rename** — ensures cleanup survives `JoinHandle::abort()` and no SDK is leaked on a failed rename; the empty outer `.fdemon-install-tmp-<pid>` wrapper directory is removed on success; `reclaim_stale_flutter_tmps` / `reclaim_stale_android_tmps` glob-remove any leftover `.fdemon-install-tmp-*` dirs under the install root (called under the lock at install start) for cross-PID reclamation; the pre-extraction disk check is not double-counted on top of the pre-download check. XZ decode threads self-terminate on `BrokenPipe` when the consumer side cancels. |
| `toolchain/process_stream.rs` | `run_streaming` — merges stdout and stderr of a long-running child process into a single ordered stream of lines, for streaming git-clone and similar operations. Phase 3 adds `run_streaming_with_input` — identical but pipes `stdin_data` to the child before closing stdin (EOF), enabling non-interactive license acceptance (`sdkmanager --licenses`); accepts extra `env` pairs (e.g. `JAVA_HOME`) injected into the child's environment. |
| `toolchain/flutter_install.rs` | Managed Flutter SDK install: `install_flutter` (git-clone default with `--` option terminator and channel validation before invocation, archive-download fallback honoring the configured channel in the archive path), atomic temp-dir-then-rename via `TempDirGuard` RAII (cleanup survives `JoinHandle::abort()`), non-fatal `flutter precache`. Reclaims incomplete `final_dir` partials on retry; `reclaim_stale_flutter_tmps` removes any leftover temp dirs from previous (possibly different-PID) runs at install start under the advisory lockfile. Serializes concurrent installs via an advisory lockfile (`LockGuard`) created atomically under the install root. `git_install` accepts and honours the `CancellationToken` — a cancelled clone returns `Error::Cancelled` cooperatively (git child is killed via `kill_on_drop(true)`). Manifest fetches carry timeouts; `FVM_CACHE_PATH` is validated to be absolute before use. Helpers: `fetch_release_manifest`, `archive_download_url`, `resolve_install_dir`. Event type: `InstallEvent`. |
| `toolchain/path_config.rs` | Idempotent, marker-fenced PATH export writes to shell rc files for bash, zsh, and fish — one export block per rc file, guarded by begin/end markers so repeated runs are idempotent. Rc-file writes are atomic: a unique temp file (generated by `tempfile`) is written and then renamed into place; the original file's permissions are preserved (0600 stays 0600; new files are created 0600). On Windows, the PATH is written directly to the registry via raw `HKCU:\Environment` access (`GetValue(..., 'DoNotExpandEnvironmentNames')` + `New-ItemProperty -PropertyType`) so that the `REG_EXPAND_SZ` value type and literal `%VAR%` tokens (e.g. `%USERPROFILE%`) are preserved — the previous `[Environment]::Get/SetEnvironmentVariable` round-trip was destructive. The new PATH value is passed out-of-band via the `FDEMON_NEW_PATH` environment variable (alongside the out-of-band `FDEMON_PATH_KIND` variable that carries the registry property type), not interpolated into the script string. Phase 5 adds a best-effort `WM_SETTINGCHANGE` broadcast after writing so the new PATH takes effect in running processes without requiring a sign-out. On POSIX/fish, `bin_dir` is validated and single-quoted before being written to rc files. macOS bash selects `.bash_profile` or `.profile` based on which file exists. `add_to_path(bin_dir, shell) -> PathConfigOutcome`. `rc_file_for_shell(shell) -> Option<PathBuf>`. Phase 3 adds `add_android_env(shell, platform, sdk_root) -> Result<PathConfigOutcome>` — writes `ANDROID_HOME` and the `cmdline-tools/latest/bin` + `platform-tools` PATH entries in a separate, distinctly-fenced block; idempotent (detects and replaces the existing block if the SDK root changed); Phase 5 also appends the `WM_SETTINGCHANGE` broadcast for `ANDROID_HOME` writes on Windows. Phase 6: `add_android_env` also writes `$ANDROID_HOME/emulator` in the same PATH block (all three paths in order: `cmdline-tools/latest/bin`, `platform-tools`, `emulator`). The PathConfig step executor in `actions.rs` falls back to `resolve_android_sdk_root_path(None)` (filtered by `is_dir()`) when `settings.toolchain.android_sdk_root` is unset, so an out-of-band Android SDK installation still gets `ANDROID_HOME` and all three PATH entries written. The internal `home_dir()` helper has a `#[cfg(test)]`-only thread-local override (`TEST_HOME_OVERRIDE`, settable via `with_test_home`/`set_test_home_override`/`clear_test_home_override`) so unit tests never write to a developer's real `~/.zshenv` or `~/.zprofile`; production home resolution is unchanged. |
| `toolchain/android_install.rs` | Phase 3. `install_android_tools(target, on_event)` — full Android SDK install flow: download `cmdline-tools` zip for the host platform (verifying the optional `cmdline_tools_sha256` checksum before extraction), extract to a temp directory, relocate to `<sdk_root>/cmdline-tools/latest` via atomic backup-restore rename (renames existing `latest/` to `latest.bak-<pid>`, restores on failure, removes backup on success), accept SDK licenses non-interactively via `sdkmanager --licenses` (feeds `y\n` through stdin using `run_streaming_with_input`), then install the required packages via `sdkmanager`. The sdkmanager child process PATH is assembled with `std::env::split_paths`/`join_paths` (OS-correct separator) rather than a hardcoded `:`. All progress reported as `InstallEvent` callbacks. Returns `AndroidInstallOutcome`. `resolve_cmdline_tools_url(target)` is the testable URL-builder helper. |
| `toolchain/jdk.rs` | Phase 3. `resolve_jdk_home() -> Option<PathBuf>` — best-effort JDK home resolution: checks `$JAVA_HOME` first, then walks from the `java` binary found via `which` (`java_home_from_which`). The `which`-walk rejects stub paths under `/usr` and `/usr/local` and requires a real JDK marker (`release` file or `bin/javac`) before accepting the resolved home — this prevents system-installed Java stubs from being mistaken for a full JDK. `configure_flutter_jdk_dir(flutter, jdk_dir)` — runs `flutter config --jdk-dir=<dir>` so the Flutter CLI uses the specified JDK when building Android targets. |

**Flutter SDK Detection (`flutter_sdk/`):**

`find_flutter_sdk()` runs up to 12 ordered strategies (explicit config, environment variables, version managers, system PATH, binary-only shim fallback). Strategy 10 uses `which::which("flutter")` which respects `PATHEXT` on Windows to correctly locate `flutter.bat`, `flutter.cmd`, or `flutter.exe`. Path normalization uses `dunce::canonicalize` instead of `std::fs::canonicalize` to avoid `\\?\`-prefixed UNC paths that `cmd.exe` cannot consume.

| # | Strategy | Description |
|---|----------|-------------|
| 1 | Explicit config | `[flutter] sdk_path` in `.fdemon/config.toml` |
| 2 | `FLUTTER_ROOT` env | Environment variable set by the user or CI |
| 3 | FVM modern | `.fvmrc` in project tree |
| 4 | FVM legacy | `.fvm/fvm_config.json` + symlink |
| 5 | Puro | `.puro.json` in project tree |
| 6 | asdf | `.tool-versions` in project tree |
| 7 | mise | `.mise.toml` in project tree |
| 8 | proto | `.prototools` in project tree |
| 9 | flutter_wrapper | `flutterw` script + `.flutter/` directory |
| 10 | System PATH | `which::which("flutter")` → resolve symlinks → SDK root |
| 11 | Lenient PATH fallback | Binary on PATH but `VERSION` file missing or unreadable |
| 12 | Binary-only fallback (shim-installer support) | Last resort. When `which::which("flutter")` succeeds but the inferred SDK root fails both strict and lenient validation, returns a `FlutterSdk` with `source = SdkSource::PathInferred`, `version = "unknown"`. This unblocks scoop and winget Flutter installations that don't follow the canonical `<root>/bin/flutter` layout. |

**Diagnostic hints are content-gated.** `devices.rs` and `emulators.rs` only append the Windows-specific `windows_hint()` (which directs users to set `[flutter] sdk_path` in `.fdemon/config.toml`) when the failure's stderr matches a path-resolution error pattern (via `is_path_resolution_error()`). This prevents the hint from misleading users when `flutter` exits non-zero for unrelated reasons (e.g., adb crashed, license not accepted, network proxy errors).

`FlutterExecutable` has two variants:

| Variant | When produced | Runtime invocation |
|---------|---------------|--------------------|
| `Direct(PathBuf)` | Unix or Windows `.exe` | `Command::new(path)` |
| `WindowsBatch(PathBuf)` | Windows `.bat` / `.cmd` | `Command::new(path)` |

Both variants invoke the resolved absolute path directly via `Command::new`. The `WindowsBatch` discriminant is a metadata marker (callers and logs can distinguish batch from native executables) — the runtime invocation is identical to `Direct`. The previous `cmd /c <path>` wrapper has been removed; direct invocation is safe because the workspace MSRV is 1.77.2, which includes the CVE-2024-24576 fix for `.bat` argument escaping.

**Platform Support:**

| Platform | Mechanism          | Module        |
|----------|--------------------|---------------|
| Android  | `adb logcat`       | `android.rs`  |
| macOS    | `log stream`       | `macos.rs`    |
| iOS (sim)| `simctl log stream`| `ios.rs`      |
| iOS (phy)| `idevicesyslog`    | `ios.rs`      |
| Others   | Not needed (pipe)  | —             |

**Tool Dependencies:**
- `adb` — Android Debug Bridge, required for Android logcat capture
- `log` — macOS unified logging tool, required for macOS native log capture
- `xcrun simctl` — Xcode CLI tools, required for iOS simulator log capture
- `idevicesyslog` — part of the `libimobiledevice` suite, required for physical iOS device log capture (optional; graceful degradation if absent)

**Key Protocol:**
- Flutter's `--machine` flag outputs JSON-RPC over stdout
- Messages wrapped in `[...]` brackets
- Events: `daemon.connected`, `app.start`, `app.log`, `device.added`, etc.
- Commands: `app.restart`, `app.stop`, `daemon.shutdown`, etc.

### `fdemon-app` — Application State and Orchestration

**Location**: `crates/fdemon-app/`
**Dependencies**: `fdemon-core`, `fdemon-daemon`; `reqwest` (rustls-tls) for the GitHub version check HTTP call
**Purpose**: TEA pattern implementation, Engine orchestration, services, config, watcher

**Core Modules:**

| File | Purpose |
|------|---------|
| `engine.rs` | `Engine` struct — shared orchestration core for TUI and headless runners |
| `engine_event.rs` | `EngineEvent` enum — domain events broadcast to external consumers |
| `state.rs` | `AppState` — complete application state (the Model) |
| `message.rs` | `Message` enum — all possible events/actions |
| `signals.rs` | Signal handling for SIGINT/SIGTERM |
| `handler/` | `update()` function and handler helpers (TEA) |
| `handler/mouse/` | Per-mode mouse event handlers; `mod.rs` dispatches by `UiMode`, sub-modules handle scroll and click hit-testing per mode |
| `input_mouse.rs` | `MouseInput`, `MouseButton`, `ScrollDir`, `KeyModSet` — raw mouse event types |
| `mouse_regions.rs` | Per-frame click-region registry (`MouseRect`, `MouseAction`, `MouseRegionEntry`, `MouseRegions`, `MouseRegionsBuilder`, `MouseRegionsCell`, `MouseRegionGuard`). `fdemon-app` does **not** depend on `ratatui`; `MouseRect` mirrors `ratatui::layout::Rect` locally, with conversion handled at the `fdemon-tui` boundary. |
| `session/` | `Session`, `SessionHandle`, per-session state: `PerformanceState`, `MemoryState`, `NetworkState`, `NativeTagState` |
| `session_manager.rs` | `SessionManager` — manages up to 9 concurrent sessions |
| `watcher.rs` | `FileWatcher` — watches `lib/` for `.dart` changes, debounces, emits `WatcherEvent` |
| `version_check.rs` | GitHub releases API client; queries the latest fdemon release at TUI startup and returns `Some(version)` when a newer release is available. Results are cached in `<dirs::cache_dir()>/fdemon/version_check.json` (24 h TTL, JSON `{ checked_at, latest }`; per-user, not per-project) — no outbound request is made on a cache hit within the TTL. Errors and non-newer releases collapse silently to `None` (fire-and-forget — this is a developer convenience, not a security channel). |

**Configuration (`config/`):**

| File | Purpose |
|------|---------|
| `types.rs` | `LaunchConfig`, `Settings`, `FlutterMode`, and related types. Phase 2 adds `ToolchainSettings` — the `[toolchain]` config block controlling managed Flutter install method, channel, install directory, and Android SDK paths. |
| `settings.rs` | Loads `.fdemon/config.toml` for global settings |
| `launch.rs` | Loads `.fdemon/launch.toml` for launch configurations |
| `vscode.rs` | Parses `.vscode/launch.json` for VSCode compatibility |

**Configuration Files:**
- `.fdemon/config.toml` — Behavior, watcher, UI settings
- `.fdemon/launch.toml` — Launch configurations (device, mode, flavor, etc.)
- `.vscode/launch.json` — VSCode Dart launch configs (auto-converted)

**Services (`services/`):**

The services layer provides trait-based abstractions for Flutter control operations, managed by the Engine. `FlutterController`, `LogService`, and `StateService` carry data and are held on `AppState` or in `SharedState`. `Clipboard` is the first service in the family that is side-effect-only — it is owned by the TUI runner (not held on `AppState`) and is invoked via `AppState::pending_runner_actions`, preserving TEA purity.

| File | Purpose |
|------|---------|
| `flutter_controller.rs` | `FlutterController` trait — `reload()`, `restart()`, `stop()`, `is_running()` |
| `log_service.rs` | `LogService` trait — log buffer access and filtering |
| `state_service.rs` | `SharedState` — thread-safe state with `Arc<RwLock<>>` |
| `clipboard.rs` | `Clipboard` trait — cross-platform clipboard writer; `SystemClipboard` (arboard-backed, used at runtime), `MemoryClipboard` (in-memory, used in tests). Owned by the TUI runner, not held on `AppState`, preserving TEA purity for this side-effect-only service. |

**UI State:**

| File | Purpose |
|------|---------|
| `editor.rs` | `open_in_editor()` function for file navigation |
| `settings_items.rs` | Setting item generators for settings panel |
| `log_view_state.rs` | `LogViewState` — scroll/viewport state |
| `hyperlinks.rs` | `LinkHighlightState` — link detection and navigation |
| `confirm_dialog.rs` | `ConfirmDialogState` — confirmation dialog state |
| `new_session_dialog/` | New session dialog state (fuzzy filtering, target selector, device groups) |
| `install_wizard/` | `InstallWizardState`, `WizardStep`, `WizardStepKind`, `StepStatus`, `WizardPane`, `build_steps()` — state types for the two-pane toolchain diagnostics modal. Phase 2 adds `StepExecution` and `StepExecStatus` (tracks Idle/Running/Succeeded/Failed install state and log tail on `InstallWizardState.execution`) plus `installed_sdk_path`. Phase 5 followup adds `StepExecStatus::Cancelled` — a terminal state for user-initiated cancellations distinct from `Failed`; renders with a neutral glyph and no red styling, and the step remains retriable via Enter. (stashed path from a just-completed FlutterSdk step, cleared after the PathConfig step consumes it). `StepExecution::log_tail` is a bounded `VecDeque<String>` storing raw lines; O(1) front-eviction via `pop_front`. ANSI codes are stripped at render time by the `StepProgress` widget in `progress.rs`, not before storage. Phase 3 adds `GuidedCommand` (`label`, `command`, `note`) — a copy-paste command shown for privileged/GUI actions the wizard cannot auto-run; `WizardStep.guided_commands: Vec<GuidedCommand>` populated by `build_steps()` (e.g. JDK install command when the JDK component is not `Ok`); `InstallWizardState::selected_guided_command()` returns the command at `selected_command_index` of the selected step for the `c` key handler. Phase 4 adds `selected_command_index: usize` to `InstallWizardState` (defaults to 0, reset on step change and `apply_report`); `select_next_command()` / `select_prev_command()` advance/retreat the index (clamped/saturating, no-op for steps with 0 or 1 commands); `selected_guided_command()` is now index-aware; `prerequisites_guided_commands(report, components)` generates per-OS install commands for the Prerequisites step as a pure function of `ToolchainReport` (reads `report.linux_package_manager` and `report.winget_available` — no `which::which` I/O in the TEA `update()` path). Phase 5 adds `install_task: Option<InstallTaskHandle>` to `InstallWizardState` — an `InstallTaskHandle` bundles the `CancellationToken` and `JoinHandle` for the in-flight install task; populated by `WizardInstallTaskReady` and cleared when the step completes, fails, or is cancelled. `flutter_now_live() -> bool` predicate returns `true` when the preflight report shows Flutter as `Ok`. `handback_done: bool` one-shot guard prevents duplicate device-discovery dispatches. Phase 6: `jdk_guided_command` takes `&ToolchainReport` and dispatches the JDK package name on `report.linux_package_manager` (pacman → `jdk17-openjdk`, dnf → `java-17-openjdk-devel`, yum → `java-17-openjdk-devel`, apt → `openjdk-17-jdk`, zypper → `java-17-openjdk-devel`, Unknown → Adoptium URL), matching the same package-manager dispatch already used by `prerequisites_guided_commands`. `prerequisites_guided_commands` (Linux path) filters the install command to only-missing packages via `parse_missing_prereq_keys`, mapping each key to the distro-specific package name via the `linux_package_name` table (covers git, zip, curl, unzip, xz, clang, cmake, ninja, pkg-config, libgtk-3-dev, libglu1-mesa, libstdc++). Phase 7: `apply_report` now also resets `execution` to `StepExecution::default()`, so pressing `r` to re-check after a Failed or Cancelled step shows the refreshed component list rather than the stale Failed/Cancelled view. |
| `handler/install_wizard/` | Navigation and action handlers for `UiMode::InstallWizard`. `mod.rs` is a re-export shim; `navigation.rs` holds the up/down and pane-switch handlers. Phase 2 adds `actions.rs` — handles `WizardStepStarted/Log/Progress/DownloadProgress/Completed/Failed/Phase` messages, chains `PersistSettings` → re-run preflight → `ScanInstalledSdks` on `WizardStepCompleted(FlutterSdk)`. Phase 3 extends `actions.rs`: `AndroidTools` step is now executable but gated — dispatches `RunWizardStep` only when the JDK preflight component is `Ok`, otherwise surfaces a status message pointing the user to the guided JDK install command; `PathConfig` step executor now also calls `add_android_env` when an `android_sdk_root` is present; `InstallWizardCopyCommand` message handler copies `selected_guided_command()` to the system clipboard. Phase 4 extends `navigation.rs` with `handle_prev_command` / `handle_next_command` (bound to `[` / `]` keys, delegate to `select_prev_command()` / `select_next_command()`, work regardless of focused pane, reset `selected_command_index` to 0 on step list navigation). Phase 4 extends `actions.rs`: `Prerequisites` Enter arm is split from `Doctor` — the stub "Available in a later phase" message is replaced with the guided "Run the listed command(s), then press r to re-check." message; `Doctor` Enter still shows the "later phase" stub. Phase 5 extends `actions.rs`: `handle_cancel_step` cancels the token on `InstallWizardState.install_task` and resets execution to Idle; `handle_preflight_completed` checks `flutter_now_live()` and, when true and `handback_done` is unset, auto-closes the wizard and dispatches `DiscoverDevices`; `Esc` / `HideInstallWizard` with a live SDK also dispatches `DiscoverDevices` and routes to `UiMode::Startup`; `RunToolchainPreflight` emits `SdkResolved` so `flutter_executable()` resolves after a managed install. Auto-PATH-config: `handle_step_completed` now emits `Message::InstallWizardAutoConfigurePath { kind }` (alongside `PersistSettings`) on a successful `FlutterSdk` or `AndroidTools` completion. `handle_auto_configure_path` in `actions.rs` handles that message: it calls `begin_step(PathConfig)` (minting a fresh `CancellationToken` and bumping `run_seq` for seq-guard compliance) and dispatches `UpdateAction::RunWizardStep { kind: PathConfig, .. }`. The `FlutterSdk`-origin auto-config carries `android_sdk_root: None` (Flutter PATH only); the `AndroidTools`-origin auto-config carries the resolved `android_sdk_root` so `ANDROID_HOME` and the Android PATH entries are written. PathConfig completion (success or failure) re-runs preflight so the step list refreshes. No loop: PathConfig completion does not re-emit `InstallWizardAutoConfigurePath`. |

**Message Categories:**
- Keyboard events (`Key`)
- Daemon events (`Daemon`)
- Scroll commands (`ScrollUp`, `ScrollDown`, etc.)
- Control commands (`HotReload`, `HotRestart`, `StopApp`)
- Session management (`NextSession`, `CloseCurrentSession`)
- Device/emulator management (`ShowDeviceSelector`, `LaunchEmulator`)
- Install wizard step execution: `InstallWizardRunSelectedStep` (Enter on a runnable step), `WizardStepStarted { kind, run_seq }`, `WizardStepLog { kind, line }`, `WizardDownloadProgress { kind, received, total }`, `WizardStepCompleted { kind, summary, sdk_path }`, `WizardStepFailed { kind, reason }`, `WizardStepPhase { kind, label }` — routes to `handle_step_phase` → `set_step_phase` to drive the live phase row in the `StepProgress` widget. `WizardStepStarted` carries `run_seq: u64`; `handle_step_started` is sequence-aware — a message whose `kind` or `run_seq` does not match the current run is a pure no-op and cannot drop the live `install_task`, bump `run_seq`, or desync the UI (the old defensive `begin_step` fallback was removed). Phase 3 adds `InstallWizardCopyCommand` — copies the selected step's first `GuidedCommand.command` to the system clipboard (bound to `c`). Phase 5 adds `InstallWizardCancelStep` — dispatched by `Esc` while a step is Running; fires the synchronously-stored `CancellationToken` and resets execution to Idle. `WizardInstallTaskReady { kind, run_seq, handle }` — sent by the executor after `tokio::spawn` returns (the only point the `JoinHandle` is available); the handler validates `kind` and `run_seq` against the current run before upgrading the stored handle's `join` field; stale or mismatched readies have their handle aborted and are otherwise discarded. The `CancellationToken` is stored synchronously by `handle_run_selected_step` before the action is dispatched (in `InstallTaskHandle { cancel, join: None }`), so `Esc` can fire it at any point after the step transitions to Running — including the window before `WizardInstallTaskReady` arrives. `handle_step_failed` distinguishes `Error::Cancelled` (routes to `StepExecStatus::Cancelled`, neutral rendering) from other errors (routes to `StepExecStatus::Failed`, red "Failed" styling with retry hint).

### `fdemon-tui` — Terminal UI (Presentation Layer)

**Location**: `crates/fdemon-tui/`
**Dependencies**: `fdemon-core`, `fdemon-app`
**Purpose**: Presentation layer using `ratatui`. The TUI runner creates an Engine and uses it for all state management.

**Note on daemon display types**: `fdemon-tui` has no runtime dependency on `fdemon-daemon`. Toolchain display types needed by the install-wizard widgets (`ComponentCheck`, `ComponentKind`, `ComponentStatus`, `DoctorLine`, `DoctorMarker`, `HostPlatform`, `HostShell`, `LinuxPackageManager`, `ToolchainReport`) are re-exported through `fdemon-app::install_wizard`, so presentation widgets consume them via `fdemon-app` rather than reaching into `fdemon-daemon` directly. `fdemon-daemon` appears only in `fdemon-tui`'s `[dev-dependencies]` for test helpers.

**Key Architecture:**
- **Runner** (`runner.rs`): Main entry point, creates Engine, runs event loop
- **Event Polling** (`event.rs`): Polls terminal for keyboard/resize events, converts to `Message`
- **Rendering** (`render/`): Renders `AppState` to terminal using ratatui widgets
- **Widgets** (`widgets/`): Reusable UI components (header, tabs, log view, status bar, dialogs)

| File | Purpose |
|------|---------|
| `runner.rs` | Main entry point, Engine creation, event loop |
| `startup.rs` | TUI-specific startup logic |
| `render/mod.rs` | State → UI rendering; defines `MouseCtx` — the per-frame borrowed bridge that threads `MouseRegionsBuilder` into widgets |
| `render/tests.rs` | Full-screen snapshot and transition tests |
| `layout.rs` | Layout calculations for different UI modes |
| `event.rs` | Terminal event polling (keyboard, resize) |
| `terminal.rs` | Terminal initialization, cleanup, panic hook |
| `selector.rs` | Interactive project selection (when multiple found) |
| `test_utils.rs` | TestTerminal wrapper and test helpers |

**Widgets (`widgets/`):**

| Widget | Purpose |
|--------|---------|
| `header.rs` | Application title bar with project name |
| `tabs.rs` | Tab bar for multi-session navigation (1-9) |
| `log_view/` | Scrollable log display with syntax highlighting |
| `status_bar.rs` | Bottom bar showing phase, device, reload count |
| `device_selector.rs` | Modal for device/emulator selection |
| `settings_panel/` | Settings editor (project, user prefs, launch configs, VSCode) |
| `confirm_dialog.rs` | Confirmation dialog widget |
| `tag_filter.rs` | Native tag filter overlay — toggle per-tag visibility, shows tag counts |
| `new_session_dialog/` | New session creation dialog |
| `install_wizard/` | Two-pane toolchain diagnostics panel: `mod.rs` (panel entry), `step_list.rs` (left pane: ordered step list; Phase 5 adds a run-failed badge `✗` on failed steps), `step_detail.rs` (right pane: per-step detail with Enter action hints for runnable steps; Phase 3 adds guided-command block — label, command text, optional alternative note, and `c` key copy hint), `doctor_view.rs` (embedded `flutter doctor -v` output view), `progress.rs` (Phase 2: `StepProgress` widget — live progress bar, byte counter, and scrolling log tail shown while a step is running; Phase 5 adds "Esc cancels" hint in the `StepProgress` footer while a step is Running) |

### `fdemon-dap` — DAP Server

**Location**: `crates/fdemon-dap/`
**Dependencies**: `fdemon-core` only
**Purpose**: Debug Adapter Protocol implementation — TCP server, protocol types, adapter logic, stdio transport

**Key Design Constraint**: `fdemon-dap` has no dependency on `fdemon-daemon` or
`fdemon-app`. The `DebugBackend` trait abstracts all VM Service operations;
`fdemon-app` provides the concrete `VmServiceBackend` implementation.

| Module | Purpose |
|--------|---------|
| `protocol/types.rs` | All DAP request, response, and event types — includes `RestartFrameArguments`, `ExceptionInfoArguments`, `BreakpointLocationsArguments`, `BreakpointLocation`, `CompletionsArguments`, `CompletionItem` |
| `protocol/codec.rs` | Content-Length framing encoder/decoder |
| `adapter/mod.rs` | `DapAdapter` struct, `ExceptionRef` type; re-exports from sub-modules |
| `adapter/backend.rs` | `DebugBackend` / `LocalDebugBackend` trait, `DynDebugBackend` wrapper, `BackendError` |
| `adapter/handlers.rs` | `handle_request` dispatch and all per-command handler methods |
| `adapter/breakpoints.rs` | `BreakpointState` — DAP ID ↔ VM ID mapping, conditional breakpoints, logpoints |
| `adapter/variables.rs` | Variable expansion, type rendering (Record, WeakReference, Sentinel, truncated strings, Set), getter evaluation, `toString()` display enrichment, `evaluateName` construction |
| `adapter/evaluate.rs` | Expression evaluation handler, `EvalContext` (hover/watch/repl/clipboard) |
| `adapter/events.rs` | Event emission helpers — progress start/end, custom event forwarding |
| `adapter/stack.rs` | `FrameStore`, `VariableStore` (with `MAX_VARIABLE_REFS` cap), `SourceReferenceStore`, scope kinds (Locals, Globals, Exception) |
| `adapter/threads.rs` | `ThreadMap`, `MultiSessionThreadMap`, session ID namespacing |
| `adapter/types.rs` | `StepMode` (including `Rewind`), `DapExceptionPauseMode`, `BreakpointResult`, `DebugEvent`, `PauseReason`, `REQUEST_TIMEOUT` |
| `server/mod.rs` | `DapServer` — TCP accept loop, client session spawning |
| `server/session.rs` | `DapClientSession`, `NoopBackend` (test-only backend) |
| `transport/stdio.rs` | Stdio transport for IDE integration testing |

### `flutter-demon` (Binary) — CLI and Headless Mode

**Location**: `src/`
**Dependencies**: `fdemon-core`, `fdemon-daemon`, `fdemon-app`, `fdemon-tui`, `fdemon-dap`
**Purpose**: Binary entry point, CLI parsing, headless NDJSON mode, `doctor` subcommand

**CLI Surface:**

The binary uses a clap default-subcommand idiom to preserve the existing `fdemon /path`, `--headless`, and `--dap-*` flag surface while also exposing named subcommands.

| Subcommand / flag | Purpose |
|-------------------|---------|
| `fdemon [path]` *(default)* | Launch TUI for the Flutter project at `path` (or cwd if omitted) |
| `fdemon doctor` | Run a read-only toolchain preflight check (`src/doctor.rs`), print `[STATUS] kind — detail` lines (status column is fixed-width, right-aligned to 4 chars) plus captured `flutter doctor` output to stdout. Exit 0 when all components relevant to the current project are healthy; exit 1 when any required component is degraded. Android-specific components (Android SDK, build-tools, platforms, licenses, etc.) only gate the exit code when an Android SDK is actually present — a pure Flutter project without Android configured exits 0 even if Android components are missing. Doctor-incompatible top-level flags (`--headless`, `--dap-stdio`, `--dap-port`, `--log-dir`, `--dap-config`) are rejected at startup with exit 2 instead of being silently ignored. Loads project settings and honours the configured `[flutter] sdk_path` — the same SDK resolution path used by the TUI engine. Note: because `doctor` is a named clap subcommand, a Flutter project in a directory literally named `doctor` cannot be launched via the bare token `fdemon doctor`; use `fdemon ./doctor` as a workaround. |
| `--headless` | Non-TUI NDJSON mode (existing) |
| `--dap-*` | DAP server flags (existing) |

`src/doctor.rs` calls `fdemon_daemon::toolchain::run_preflight()` (the same entry point used by the install wizard) and renders `ComponentStatus` as a fixed-width string via `.to_string()` with a `{:>4}` format specifier so all status labels align. `main.rs` loads `fdemon_app::config::load_settings(&cwd)` in the `Doctor` branch and passes `settings.flutter.sdk_path` as the `explicit_sdk` parameter to `run_doctor`, mirroring the engine's SDK resolution path. `fdemon setup` (managed-install CLI) is deferred.

**Headless Mode:**

Headless mode provides a non-TUI interface for E2E testing and automation. It creates an Engine and outputs structured NDJSON events to stdout.

| File | Purpose |
|------|---------|
| `mod.rs` | `HeadlessEvent` enum and NDJSON serialization |
| `runner.rs` | Headless runner, Engine creation, stdin reader, event loop |

**HeadlessEvent Types:**
- `DaemonConnected`, `DaemonDisconnected`
- `AppStarted`, `AppStopped`
- `HotReloadStarted`, `HotReloadCompleted`, `HotReloadFailed`
- `Log`, `Error`
- `SessionCreated`, `SessionRemoved`

---

## Key Patterns

### TEA Message Flow (via Engine)

The Engine acts as the central hub for all message processing. Both TUI and headless runners send messages to the Engine, which processes them through the TEA update cycle.

```
┌──────────────────────────────────────────────────────────────────┐
│                          Event Loop                              │
│                                                                  │
│  Input Sources                     Engine                        │
│  ┌─────────┐                  ┌──────────────┐                  │
│  │ Terminal│─────┐            │ msg_channel  │                  │
│  │  Event  │     │            │      ↓       │                  │
│  └─────────┘     │            │ process_msg  │                  │
│                  ├───Message──▶│      ↓       │                  │
│  ┌─────────┐     │            │  update()    │───Action────┐    │
│  │ Daemon  │─────┤            │      ↓       │             │    │
│  │  Event  │     │            │  AppState    │             ▼    │
│  └─────────┘     │            │      ↓       │      handle_action() │
│                  │            │emit_events() │      sync_shared_state() │
│  ┌─────────┐     │            └──────┬───────┘             │    │
│  │ Watcher │─────┤                   │                     │    │
│  │  Event  │     │                   ▼                     ▼    │
│  └─────────┘     │            EngineEvent            UpdateAction│
│                  │            (broadcast)            (side effects)│
│  ┌─────────┐     │                                                │
│  │ Signal  │─────┘                                                │
│  │ Handler │                                                      │
│  └─────────┘                                                      │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ TUI Runner: Render after drain_pending_messages()       │    │
│  │ Headless Runner: Emit NDJSON events after process_msg() │    │
│  └─────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘
```

**Message Processing Steps:**
1. Input source (terminal, daemon, watcher, signal) sends `Message` to Engine's channel
2. Engine calls `process_message(msg)`:
   - Captures state snapshot (pre)
   - Calls `handler::update(state, msg)` → returns `(new_state, action)`
   - Calls `handle_action(action)` → spawns tasks, updates SharedState
   - Captures state snapshot (post)
   - Calls `emit_events(pre, post)` → broadcasts `EngineEvent` to subscribers
3. Runner-specific handling:
   - **TUI**: Drains all messages, flushes logs, renders frame
   - **Headless**: Processes one message, flushes logs, emits NDJSON

### Multi-Session Architecture

```
SessionManager
├── sessions: HashMap<SessionId, SessionHandle>
├── session_order: Vec<SessionId>  (for tab ordering)
└── selected_index: usize

SessionHandle
├── session: Session  (state)
├── process: Option<FlutterProcess>
├── cmd_sender: Option<CommandSender>
├── request_tracker: Arc<RequestTracker>
├── vm_shutdown_tx / vm_request_handle  (VM Service connection)
├── perf_shutdown_tx / perf_task_handle  (performance monitoring task)
├── perf_pause_tx: Option<Arc<watch::Sender<bool>>>  (pause/resume perf polling)
├── alloc_pause_tx: Option<Arc<watch::Sender<bool>>>  (pause/resume allocation profile polling)
├── network_shutdown_tx / network_task_handle  (network monitoring task)
├── network_pause_tx: Option<Arc<watch::Sender<bool>>>  (pause/resume network polling)
├── debug_shutdown_tx / debug_task_handle  (DAP debug event task)
├── timeline_shutdown_tx / timeline_pause_tx / timeline_task_handle  (timeline polling task — Phase 3)
├── native_log_shutdown_tx / native_log_task_handle  (platform capture task)
├── native_tag_state: NativeTagState  (discovered tags + visibility)
└── custom_source_handles: Vec<CustomSourceHandle>  (per-source handles)

Session
├── id, name, phase
├── current_progress: Option<String>   (latest launch progress line from app.progress)
├── device_id, device_name, platform
├── logs: Vec<LogEntry>
├── log_view_state: LogViewState
├── app_id: Option<String>
└── reload_count, timing data
```

### Request/Response Tracking

```
CommandSender
    │
    ▼
DaemonCommand ──┬──▶ RequestTracker.register(id)
    │           │
    ▼           │
stdin.write()   │
    │           │
    ▼           │
FlutterProcess  │
    │           │
    ▼           │
stdout ─────────┴──▶ DaemonMessage::Response
                         │
                         ▼
                    RequestTracker.complete(id)
```

### Pre-App Source Gating

`handle_launch()` conditionally returns `SpawnPreAppSources` when one or more custom sources have `start_before_app = true`. Readiness checks run concurrently with independent timeouts. The Flutter launch gate lifts on `Message::PreAppSourcesReady` (all checks passed or timed out). Sources without a `ready_check` are spawned and immediately considered ready (fire-and-forget). This pattern keeps `handle_launch()` pure (returns an action, spawns nothing directly) and routes all side effects through the normal `UpdateAction` pipeline.

### Mouse Region Registry

The mouse region registry (`fdemon-app/mouse_regions.rs`) is a per-frame, z-index-aware hit-test table. Widgets push click regions during render; the click handler reads them on button press. It lives on `AppState` as a `MouseRegionsCell` field — a `Cell<MouseRegions>` newtype that satisfies `#[derive(Debug)]` while providing interior mutability (see the TEA exception note below).

**Key types:**

- `MouseRect` — Terminal cell coordinate rectangle, mirroring `ratatui::layout::Rect` without the dependency. `fdemon-app` does not depend on `ratatui`; conversion from `Rect` to `MouseRect` is handled at the `fdemon-tui` boundary.
- `MouseAction` — Either `Emit(Box<Message>)` (a fixed message) or `EmitWithCoord(fn(u16, u16) -> Message)` (a message computed from click coordinates). `Box` keeps the enum pointer-sized.
- `MouseRegionEntry` — A single entry: `rect`, optional `on_left`, optional `on_middle`, and `z_index`.
- `MouseRegions` — Backing `Vec<MouseRegionEntry>`. `hit_test(x, y, button)` returns the highest-z entry whose rect contains the point and has a binding for the given button; ties at the same `z_index` are broken by last-pushed-wins (later render order wins).
- `MouseRegionsBuilder` — Borrowed builder with `click(rect, action)`, `click_at_z(rect, action, z)`, and `click_left_middle(rect, left, middle)` helpers. Empty rects are silently skipped.
- `MouseRegionsCell` — Thin newtype wrapping `Cell<MouseRegions>` with `take_guard()` (canonical accessor), `take()`, `set()`, and a custom `Debug` impl. Production code should use `take_guard()` (see below); `take()` and `set()` remain as low-level primitives available for tests.
- `MouseRegionGuard<'a>` — RAII guard returned by `MouseRegionsCell::take_guard()`. Holds the `MouseRegions` taken from the cell; on `Drop`, puts it back automatically. Exposes `MouseRegions` via `Deref`/`DerefMut`.

**Per-frame lifecycle (render path):**

1. `render::view()` calls `state.mouse_regions.take_guard()` at frame start. The guard takes ownership of the inner `MouseRegions` (leaving `Default::default()` in the cell for the duration of the frame) and exposes it via `Deref`/`DerefMut`.
2. `regions.clear()` resets the entry list while preserving the `Vec`'s allocated capacity (allocation-free at steady state).
3. `MouseCtx::new(regions.builder())` constructs the per-frame thread-through. Widgets that have clickable surfaces accept `Option<&mut MouseCtx<'_>>` and call `ctx.click(...)`, `ctx.click_at_z(...)`, or `ctx.click_left_middle(...)` as they paint. Passing `None` keeps widgets usable in unit tests that render without a registry.
4. When the guard goes out of scope at the end of `view()`, its `Drop` impl puts the populated registry back into the cell — no explicit `set()` call is required.

**Per-click lifecycle (handler path):**

On `Message::Mouse(MouseInput::Press { x, y, button, .. })`, `handler/mouse/mod.rs::handle_press` routes to the per-mode handler. Each per-mode handler (`normal`, `devtools`, `confirm_dialog`, `settings`, `new_session`, `link_highlight`, `tag_filter`) uses the `take_guard()` pattern: a guard wraps the hit-test, ensuring the registry is restored even if the hit-test path panics. Modes without a wired click surface (`EmulatorSelector`, `Loading`, `SearchInput`, `FlutterVersion`) return `None`.

Two gate checks sit above the per-mode dispatch in `handle_press`:

- **Tag-filter overlay gate** (`handler/mouse/mod.rs`): when `state.tag_filter_visible`, press events route directly to `tag_filter::handle_press` regardless of the underlying `ui_mode`. The overlay's row regions are registered by the tag-filter widget and resolved through the same `hit_test` path.
- **Busy gate** (`handler/mouse/normal.rs`): `HotReload`, `HotRestart`, and `StopApp` messages resolved from click regions are suppressed when `any_session_busy()` returns `true`, mirroring the equivalent check in `handler/keys.rs`.

**Modal Precedence and Sub-Modal Gates:**

When a modal `UiMode` is active (`Startup`, `NewSessionDialog`, `ConfirmDialog`, `Settings`, `FlutterVersion`, `EmulatorSelector`, `InstallWizard`) or when `tag_filter_visible` is set, `render::view()` passes `None` (instead of `Some(&mut mouse_ctx)`) to `MainHeader` and `LogView`. Base-UI z=0 regions are therefore not registered during modal frames. Per-mode dispatchers calling `regions.hit_test(x, y, button)` see only the modal widget's own regions — explicit `z_index` filtering at the dispatcher level is unnecessary.

`UiMode::LinkHighlight` is intentionally excluded from the modal gate: links are overlaid on top of the log view, and both the log-view scroll regions and the link-badge regions are expected to be interactive simultaneously.

`UiMode::Settings` renders a full-screen panel that replaces the log view entirely. Its regions are at z=0; suppressing the underlying header/log-view regions prevents ghost clicks.

The `is_modal_ui_mode()` helper that encodes this logic lives in `fdemon-tui/src/render/mod.rs`.

**Sub-modal gate (Settings):** Two modals (`dart_defines_modal`, `extra_args_modal`) can open on top of the Settings panel without changing `ui_mode`. Because `render::view()` cannot detect these sub-modals via `is_modal_ui_mode()`, `settings::handle_press` (`handler/mouse/settings.rs`) short-circuits to `None` when `state.settings_view_state.has_modal_open()` returns `true`. This prevents clicks outside the sub-modal's area from accidentally routing to underlying settings rows or tabs.

**Panic safety:**

Prior to `MouseRegionGuard`, a widget panic between `Cell::take()` and `Cell::set()` would silently leave the registry permanently empty (replaced with `Default::default()`), disabling mouse interaction for the rest of the session with no diagnostic. The guard's `Drop` impl restores the registry on stack unwind, eliminating this failure mode. The lower-level `MouseRegionsCell::{take, set}` methods remain available for tests but should not appear in production code.

**TEA exception note:**

`AppState::mouse_regions` uses `Cell` interior mutability, which is a deliberate exception to the TEA principle that the Model is immutable between update cycles. This is the same exception class as the existing `Cell<usize>` render-hint write-back on `TagFilterUiState`. The registry is purely a render-hint: it carries no business logic, participates in no state equality checks, and is not part of any `EngineEvent`. See `docs/CODE_STANDARDS.md` Principle 3 and `docs/REVIEW_FOCUS.md` "Approved TEA Exception → Current usage" for the canonical list of approved exceptions.

---

## DevTools Subsystem

The DevTools mode provides four inspection panels — Inspector, Performance, Memory, and Network — accessible by pressing `d` when a Flutter session has a VM Service connection.

### Architecture Overview

```
┌────────────────────────────────────────────────────────────────────────┐
│                          DevTools View                                  │
│                  (fdemon-tui/widgets/devtools/)                         │
│  ┌────────────┐  ┌───────────────┐  ┌─────────────┐  ┌─────────────┐  │
│  │ Inspector  │  │  Performance  │  │   Memory    │  │   Network   │  │
│  │ tree_panel │  │  frame_chart/ │  │MemoryPanel  │  │request_table│  │
│  │layout_panel│  │  details/     │  │ (memory/)   │  │req_details  │  │
│  │  details/  │  │               │  │             │  │             │  │
│  └──────┬─────┘  └──────┬────────┘  └──────┬──────┘  └──────┬──────┘  │
└─────────┼───────────────┼─────────────────┼──────────────────┼────────┘
          │               │                 │                  │
          ▼               ▼                 ▼                  ▼
┌────────────────────────────────────────────────────────────────────────┐
│                        DevTools Handlers                                │
│                  (fdemon-app/handler/devtools/)                         │
│  inspector.rs  performance/{mod,frame,details,rebuild_stats,timeline}.rs  memory.rs  network.rs │
└─────────┬───────────────┬─────────────────┬──────────────────┬────────┘
          │               │                 │                  │
          ▼               ▼                 ▼                  ▼
┌────────────────────────────────────────────────────────────────────────┐
│                       Per-Session State                                 │
│                    (fdemon-app/session/)                                │
│  InspectorState   PerformanceState   MemoryState    NetworkState        │
│  (in state.rs)    (performance.rs)   (memory.rs)    (network.rs)       │
└─────────┬───────────────┬─────────────────┬──────────────────┬────────┘
          │               │                 │                  │
          ▼               ▼                 ▼                  ▼
┌────────────────────────────────────────────────────────────────────────┐
│                       VM Service Client                                 │
│                  (fdemon-daemon/vm_service/)                            │
│  extensions/{inspector,layout,properties,performance,overlays,dumps}   │
│  performance.rs    network.rs   timeline.rs                            │
└─────────┬───────────────┬─────────────────────────────────┬────────────┘
          │               │                                 │
          ▼               ▼                                 ▼
┌────────────────────────────────────────────────────────────────────────┐
│                         Domain Types                                    │
│                       (fdemon-core/)                                    │
│  widget_tree.rs    performance.rs    network.rs                        │
└────────────────────────────────────────────────────────────────────────┘
```

### Panel State Model

DevTools state lives at two levels:

- **View state** (`DevToolsViewState` in `state.rs`): UI-level state shared across sessions — active panel (`DevToolsPanel` enum: `Inspector`, `Performance`, `Memory`, `Network`), overlay toggles, VM connection status. Reset when exiting DevTools mode. `DevToolsPanel::Inspector` is the default. `DevToolsPanel::Memory` was added in Phase 1 of the performance/memory split, placing it between Performance and Network in tab order.
- **Session state** (`PerformanceState`, `MemoryState`, `NetworkState` on `Session`): Per-session data. `PerformanceState` holds frame timing history; `MemoryState` holds memory snapshots, GC event history, allocation profile, and memory chart/table scroll state; `NetworkState` holds HTTP profile entries. All three persist across tab switches and survive DevTools mode exit.
- **Inspector state** (`InspectorState` within `DevToolsViewState`): Holds the widget tree, layout data, selected node, the `has_ever_rendered_tree` flag, the `hide_implementation_widgets` toggle, and the Details view fields (`details_open`, `details_tab: DetailsTab`, `details_node_id`, `details_context: DetailsContext`, `properties`, `render_properties`). `hide_implementation_widgets` survives `reset()` because it is a user preference; the Details fields are cleared on reset. Unlike the rest of `DevToolsViewState`, the `has_ever_rendered_tree` flag is also sticky for the session lifetime and determines whether a readiness poll is run on subsequent fetches. The active row list is produced by `inspector_rows()`, which folds contiguous chains of non-local-project wrapper widgets into a leader row when `hide_implementation_widgets == true`. `visible_nodes()` is kept as a backwards-compatible flat-tuple shim over the row builder. `selected_row() -> Option<InspectorRow<'_>>` returns the currently selected visible row along with its `RowGroup` variant — used by handler code to decide whether a row is a chain leader, member, or standalone node (see `crates/fdemon-app/src/state.rs`). `reset_details_and_groups()` is the canonical reset point for all transient details state (`details_open`, `details_node_id`, `details_tab`, `expanded_groups`, `properties`, `render_properties`); it is called after every successful tree refresh and after hot restart, because both events invalidate Dart object ids. Two cache fields track in-flight and completed properties fetches: `last_fetched_properties_node_id: Option<String>` is set when a `getProperties` round-trip succeeds (cache key — `handle_open_details` skips re-dispatch when this equals the selected node's `value_id`); `pending_properties_node_id: Option<String>` is set when a fetch is dispatched and cleared when the response arrives. The stale-response guard in `handle_inspector_properties_fetched` (and its layout counterpart `handle_layout_data_fetched`) discards responses whose `node_id` does not match `inspector.details_node_id` — the unified comparison key for both handlers. Using `details_node_id` as the single source of truth prevents a close-then-reopen race (user closes details on node A, immediately reopens on node B; A's in-flight response arrives and is discarded rather than applied to B's panel). When a stale response arrives and `pending_*_node_id` still points to that same stale node, both `pending_*_node_id` and `*_loading` are cleared so the next `handle_open_details` for the correct node can dispatch a fresh fetch. Both cache fields are cleared by both `reset()` (session switch) and `reset_details_and_groups()` (tree refresh / hot restart), mirroring the layout-fetch cache pair (`last_fetched_node_id`, `pending_node_id`).

Monitoring is panel-gated via `watch` channels stored on `SessionHandle`:

- `perf_pause_tx` — pauses the frame-timing polling loop when the user is not in DevTools; unpaused on DevTools entry, paused on DevTools exit.
- `alloc_pause_tx` — pauses the allocation profile polling loop. Entering either the Performance tab or the Memory tab sends `false` (unpause); exiting DevTools sends `true` (pause). Both tabs share the same sender so the polling task remains active whenever either panel is visible.
- `network_pause_tx` — pauses the network polling loop when the user is not on the Network tab; unpaused on Network tab entry, paused on Network tab exit.
- `timeline_pause_tx` — pauses the 1 Hz timeline event polling loop; unpaused on Performance panel entry, paused on Performance panel exit. Added in Phase 3.

### VM Service Data Flow

1. Performance monitoring starts lazily on the first DevTools entry for a session (not at VM Service connect time); network monitoring starts on the first Network tab visit; timeline polling starts on the first Performance panel entry. All tasks pause when their corresponding panel is not visible and resume when it becomes visible again.
2. Polling tasks call VM Service extensions via `VmServiceHandle`
3. Responses are parsed into domain types (`MemorySample`, `HttpProfileEntry`, etc.)
4. Results sent as `Message` variants to the Engine message channel
5. Handler functions update per-session state
6. TUI renders the updated state on the next frame

### Inspector Widget Tree Fetch

The Inspector panel fetches the widget tree through a two-phase sequence: isolate resolution followed by a readiness poll (skipped on explicit refresh).

**Flutter UI isolate resolution** (`resolve_flutter_ui_isolate` on `VmRequestHandle`):

The Dart VM may host multiple isolates (UI, worker, background). Targeting the wrong isolate produces empty or incorrect widget tree results. `resolve_flutter_ui_isolate` selects the correct isolate by:

1. Calling `getVM` to enumerate all live isolates.
2. Calling `getIsolate` on each non-system isolate.
3. Selecting the first isolate whose `extensionRPCs` list contains at least one `ext.flutter.*` entry — indicating that the Flutter framework has registered its service extensions in that isolate.
4. If no isolate has Flutter extensions yet (e.g., the app is still warming up), falls back to the first non-system isolate.

The resolved isolate ID is cached on `VmRequestHandle`. The cache is invalidated on hot restart via `invalidate_isolate_cache()` (called from the daemon hot-restart event path) and on session teardown via `clear_isolate_cache()`.

**Readiness poll** (`ReadinessPollConfig`):

Before fetching the widget tree on first load, fdemon polls `ext.flutter.inspector.isWidgetTreeReady` to confirm the Flutter framework has completed its first frame. The poll budget defaults to 2 attempts × 250 ms interval × 1 s per-call timeout (2.5 s worst case). All three parameters are configurable via `[devtools]` keys `readiness_poll_attempts`, `readiness_poll_interval_ms`, and `readiness_poll_call_timeout_ms` in `.fdemon/config.toml`.

**Fetch triggers and poll bypass** (`FetchTrigger`):

Each widget-tree fetch carries a `FetchTrigger` variant — `Initial` or `Refresh` — that controls how the fetch is handled:

- `Initial` — first load for the session; runs the full readiness poll.
- `Refresh` — user-initiated `r` press after a tree has been rendered at least once; skips the readiness poll and fetches immediately, avoiding an unnecessary 2.5 s wait when the framework is already running.

The sticky `has_ever_rendered_tree` flag on `InspectorState` gates whether `r` dispatches a `Refresh` or an `Initial` trigger.

**Tree row builder.** The rendered tree is built by `build_inspector_rows()` in `fdemon-core/widget_tree.rs`. The algorithm computes per-row metadata (`ticks` for ancestor guideline columns, `line_to_parent` for `├─`/`└─` branch ticks, `RowGroup` for chain-fold leaders and members) and folds contiguous chains of non-local-project wrapper widgets behind a `+ N more widgets` leader row when the user's `hide_implementation_widgets` toggle is on. This mirrors DevTools' `_alwaysVisible` heuristic (`createdByLocalProject || has >1 children || has siblings || is root`).

### Inspector Properties Fetch (Two-Stage Pipeline)

When the user presses Enter to open the Details view, `handle_open_details` dispatches both a `FetchLayoutData` action (existing) and a `FetchInspectorProperties` action (Phase 2). Both actions are returned together via `UpdateResult::actions_vec`, which packs them into the `action` and `extra_actions` fields of `UpdateResult` (see the UpdateResult section below). The engine processes both in the same `process_message` cycle.

`FetchInspectorProperties` (declared in `fdemon-app/handler/mod.rs`) triggers `spawn_fetch_inspector_properties` in `fdemon-app/actions/inspector/mod.rs`. This background task performs a two-stage `ext.flutter.inspector.getProperties` round-trip:

1. **Widget-level call** — issues one `getProperties` RPC for the selected widget's `value_id`. The response is an array of `DiagnosticsNode`s (one per property). `split_widget_and_render_properties` partitions this array: nodes with `property_type == "RenderObject"` go into the render bucket; all others go into the widget bucket.

2. **Render-object sub-call** — for each node in the render bucket that has a `value_id`, issues a second `getProperties` call to fetch the render object's own sub-properties (constraints, size, layer, semantics, etc.). Sub-property results are merged into the render bucket. Sub-fetch failures are logged at debug level and do not abort the action — partial data is better than no data (mirrors DevTools' `_loadPropertiesForNode` best-effort behavior).

The parsing helpers (`parse_properties_response`, `split_widget_and_render_properties`) live in `fdemon-daemon/vm_service/extensions/properties.rs`. The action layer calls `ext::GET_PROPERTIES` (`"ext.flutter.inspector.getProperties"`) directly via `VmRequestHandle::call_extension`, reusing the same `INSPECTOR_OBJECT_GROUP` used by all other inspector RPCs.

On success, `Message::DevToolsInspectorPropertiesFetched` carries `widget_properties` and `render_properties` back to the TEA handler. `handle_inspector_properties_fetched` applies the stale-response guard (keyed on `inspector.details_node_id` — see Inspector state description above), stores the results into `InspectorState`, and sets `last_fetched_properties_node_id` as the cache key. A single outer `tokio::time::timeout(PROPERTIES_FETCH_TIMEOUT, do_fetch_properties(...))` wrapper in `spawn_fetch_inspector_properties` bounds the total wall-clock time for the entire pipeline — covering isolate resolution, the initial widget call, and all sub-calls — to 10 seconds. Individual RPCs do not carry their own timeouts; the outer wrapper is the only timeout in the pipeline.

**VM Service extensions used by the Inspector:**

| Extension | Purpose |
|-----------|---------|
| `ext.flutter.inspector.isWidgetTreeReady` | Readiness poll before first tree fetch |
| `ext.flutter.inspector.getRootWidgetTree` | Fetch the full widget tree |
| `ext.flutter.inspector.getDetailsSubtree` | Fetch detailed subtree for a node |
| `ext.flutter.inspector.getSelectedWidget` | Get currently selected widget |
| `ext.flutter.inspector.disposeGroup` | Release VM Service object group |
| `ext.flutter.inspector.getLayoutExplorerNode` | Fetch layout data (constraints, size, flex info) for a node |
| `ext.flutter.inspector.getProperties` | Fetch widget properties + render-object properties (Phase 2) |

**`DiagnosticsNode` field sanitization.** All `DiagnosticsNode` string fields that reach the terminal renderer (or that may do so in future phases) are stripped of ANSI escape sequences at Serde deserialization time via `deserialize_sanitized_option_string`. The sanitized fields are:

- `description` — rendered directly in the tree view (Phase 1.5)
- `property_type` — used for render-object bucketing (Phase 2)
- `name` — rendered in properties panels
- `level` — used in `filter_and_sort_by_level` string comparisons
- `node_type` — defense-in-depth
- `style` — defense-in-depth
- `value_id` — defense-in-depth (IDs used as internal keys; sanitized to prevent future rendering bugs)
- `object_id` — defense-in-depth parity with `value_id` and other `Option<String>` fields on the struct

The field `location_id` is intentionally excluded — it is an opaque integer-valued token serialized as a string and never rendered.

### Inspector Details Tab Visibility

The Inspector Details view exposes up to three tabs — Widget Properties, Render Object, and Flex Explorer — mirroring DevTools' `DetailsTable` predicate. Tab visibility is data-driven rather than static:

- **Widget Properties** — always visible.
- **Render Object** — visible only when the selected widget's `getProperties` response contains a node with `propertyType == "RenderObject"` (i.e. `render_properties` is non-empty on `InspectorState`).
- **Flex Explorer** — visible only when the selected widget or its tree parent is `Row`, `Column`, or `Flex`. This mirrors DevTools' `isFlexLayout` predicate (`diagnostics_node.dart:487`).

The `DetailsContext` value type (in `fdemon-core/widget_tree.rs`) holds the tree-derived visibility predicates for one details session. It is computed once when the user opens the Details view (via `compute_details_context`, which performs a single depth-first walk over the widget tree), and cached on `InspectorState` as the `details_context` field. The cached value is cleared when the details view closes or when inspector state resets, and overwritten when the user opens details on a new node.

The `visible_tabs()` accessor on `InspectorState` derives the visible-tab list from `details_context` plus the current `render_properties` state. Both the handler layer and the TUI renderer consume this accessor, keeping them in sync without duplicating the predicate logic.

When a properties fetch settles (success or failure), the active tab may no longer be in the visible set. `clamp_details_tab()` on `InspectorState` is called from `handle_inspector_properties_fetched` and `handle_inspector_properties_fetch_failed` to snap the active tab back to the first visible tab (always Widget Properties). Tab cycling via `Tab`/`Shift+Tab` iterates the visible-tab list only; cycling when a single tab is visible is a no-op.

### Browser DevTools URL (Served Endpoint)

When the `B` key is pressed from DevTools mode, fdemon opens the Flutter DevTools UI in the system browser. To produce a stable, DDS-registered URL, fdemon uses the Flutter daemon's `devtools.serve` JSON-RPC method rather than constructing a URL from raw VM Service connection details.

The endpoint acquisition follows two channels:

- **Primary — `app.devTools` event**: The Flutter daemon emits this event asynchronously after the DevTools server starts. `fdemon-daemon/protocol.rs` parses the event and emits a `DaemonEvent::DevToolsServed` variant; the handler in `fdemon-app/handler/daemon.rs` stores the resolved base URL on the session.
- **Eager fallback — `devtools.serve` RPC**: When VM Service connection is established for a session, fdemon eagerly fires a `devtools.serve` JSON-RPC call via `fdemon-daemon/commands.rs`. This populates the endpoint before the user first presses `B`, avoiding a cold-start delay. Both channels write to the same `Session.devtools_endpoint` field (`{base_url: String, served_at: Instant}`), so whichever arrives first wins.

If neither channel has produced an endpoint by the time `B` is pressed, fdemon falls back to the legacy VM Service WebSocket URL and shows a recovery toast informing the user of the degraded path.

The `devtools.serve` method is available on Flutter SDK ≥ 1.22 (October 2020). On older SDKs the daemon returns a JSON-RPC `-32601 Method not found` error, which the daemon layer treats as a signal to suppress the eager-serve request and rely solely on the `app.devTools` event path or the legacy fallback.

### Performance Panel Interactivity

The Performance panel shows the frame timing chart and a details pane. Focus, scrolling, selection, and details tab state are tracked on `PerformanceState` (in `fdemon-app/src/session/performance.rs`).

**Dual-pane layout (Phase 2):**

Phase 2 introduces a dual-pane layout: the upper area holds the frame timing chart; the lower area holds the details pane. Layout decisions use four constants in `fdemon-tui/src/widgets/devtools/performance/mod.rs`:

| Constant | Value | Meaning |
|---|---|---|
| `MIN_DUAL_PANE_HEIGHT` | 18 rows | Minimum terminal height for the dual-pane layout. Below this the panel falls back to chart-only, hiding the details pane entirely. |
| `MIN_DETAILS_HEIGHT` | 8 rows | Minimum row allocation for the details pane within the dual-pane area. |
| `MIN_PHASE_BAR_WIDTH` | 40 columns | Minimum width for proportional phase bars in the Frame Analysis tab. Below this the phase bar degrades to an inline `B/L/P/R` summary. |
| `FRAME_CHART_PCT` | 55 % | Fraction of the dual-pane usable height allocated to the frame chart; the remaining ~45 % goes to the details pane. |

**Section focus (`PerfSection` enum):**

`PerfSection` has two variants — `FrameChart` and `Details` — corresponding to the frame timing bar chart and the details pane. `PerfSection::FrameChart` is the default on panel open. `Tab` and `Shift+Tab` cycle `focused_section` between them.

**Details tab (`PerfDetailsTab` enum):**

`PerfDetailsTab` (defined in `fdemon-app/src/state.rs`) has three variants. Unlike `InspectorState::visible_tabs()` which conditionally hides some Inspector tabs, the `RebuildStats` tab uses a different conditional visibility mechanism — see "Conditional RebuildStats Tab Visibility" below.

| Variant | Status | Data source |
|---|---|---|
| `FrameAnalysis` | Fully populated | Hints derived from `fdemon-core::frame_hints` |
| `RebuildStats` | Fully populated (Phase 3) | `ext.flutter.profileWidgetBuilds` stream via `Flutter.RebuiltWidgets` VM extension events |
| `TimelineEvents` | Fully populated (Phase 3) | `getVMTimeline` polled at 1 Hz via `spawn_timeline_polling` |

`PerfDetailsTab::FrameAnalysis` is the default. Pressing `]` emits `Message::PerfCycleDetailsTab { forward: true }`, pressing `[` emits `Message::PerfCycleDetailsTab { forward: false }`. Cycling wraps around all tabs; the `RebuildStats` tab is only shown in the TUI tab bar when `rebuild_stats_enabled` is `true` — see below.

**Scroll-offset model (live-edge drift):**

The frame chart uses "frames back from live edge" scroll semantics:

- `frame_chart_scroll_offset` — how many bars the frame chart has been scrolled back from the newest frame. `0` means the live edge is visible (most recent frames are at the right of the chart).

Pressing `End` (or the equivalent mouse click on the live-edge indicator) resets the offset to `0`, snapping the view back to the live edge.

**Render-hint `Cell<usize>` fields:**

Two fields on `PerformanceState` use `Cell<usize>` interior mutability:

| Field | Purpose |
|---|---|
| `frame_chart_visible_width` | Columns available in the frame chart area; used by the scroll handler to clamp the scroll offset. |
| `details_pane_visible_height` | Rows available in the details pane; used by `PgUp`/`PgDn` to page by the correct amount. |

Both default to `0` ("not yet rendered — use fallback"). These are the same approved TEA exception class as `alloc_table_visible_height` on `MemoryState`, `MouseRegions`, and `TagFilterUiState`. See `docs/CODE_STANDARDS.md` Principle 3 for the canonical definition of this pattern.

**Display refresh rate (`display_refresh_rate: f64`):**

`PerformanceState` carries a `display_refresh_rate` field (default `60.0` Hz, hard-coded). This value is passed to `fdemon-core::frame_hints::frame_hints()` when computing per-frame analysis hints. Parsing `Display.Refresh` VM Service events to support 90/120 Hz devices is still deferred — see PLAN.md §7.4. 60 Hz is a conservative default that is never wrong for the `is_janky` predicate.

**Frame history capacity:**

`DEFAULT_FRAME_HISTORY_SIZE` is 1800 frames (30 seconds at 60 FPS), up from the previous 300-frame default. This provides enough scroll-back history for meaningful post-hoc analysis of jank events.

**`fdemon-core::frame_hints` module:**

`crates/fdemon-core/src/frame_hints.rs` provides `frame_hints(frame: &FrameTiming, refresh_rate_hz: f64) -> Vec<FrameHint>`. The `FrameHint` enum has five variants:

| Variant | Condition |
|---|---|
| `OverBudget { excess_ms, budget_ms }` | Frame total exceeds `1000 / refresh_rate_hz` ms; `excess_ms` is the overage above budget. Always first when present. |
| `ShaderCompilation` | Shader compilation detected in the raster phase. |
| `LongestUiPhase { phase, share }` | One UI phase dominates; only when `phases` is `Some`. |
| `RasterDominant { ui_ms, raster_ms }` | Raster time materially exceeds UI time. |
| `BuildDominant { ui_ms, raster_ms }` | Build time materially exceeds raster time. |

This is a pure helper module with no I/O; the Frame Analysis TUI tab consumes it directly from `frame_analysis_tab.rs`. No new VM Service RPCs are needed — the Frame Analysis tab is data-complete with the existing `FrameTiming.phases` field.

**Phase 3: `PerformanceState` new fields (rebuild stats + timeline):**

| Field | Type | Purpose |
|---|---|---|
| `rebuild_stats_enabled` | `bool` | Whether `ext.flutter.profileWidgetBuilds` is currently active; drives RebuildStats tab visibility and is preserved across hot restart for re-enable. |
| `rebuild_stats_location_map` | `LocationMap` | Incrementally merged map from `Flutter.RebuiltWidgets` events and one-shot `widgetLocationIdMap` fallback; maps location id → `Location`. |
| `rebuild_stats_totals` | `HashMap<u32, u32>` | Lifetime accumulator: location id → total rebuild count since tracking was last enabled. Cleared on disable. |
| `rebuild_stats_frames` | `VecDeque<RebuildStatsSnapshot>` | Per-frame ring buffer (newest at back); capped by `settings.devtools.rebuild_stats_frame_window`. |
| `rebuild_stats_scroll_offset` | `usize` | Scroll offset for the Rebuild Stats table. |
| `rebuild_stats_selected_row` | `Option<usize>` | Currently-selected row in the Rebuild Stats table. |
| `timeline_tracks` | `BTreeMap<i64, TimelineTrack>` | Per-thread event trees, keyed by `tid` ascending; replaces the Phase 3 flat `VecDeque<TimelineEvent>` ring buffer. Iteration order is stable (`BTreeMap`) so the Gantt renderer produces consistent thread-row ordering. |
| `timeline_thread_scroll_offset` | `usize` | Scroll offset measured in thread rows (not event lines); replaces `timeline_events_scroll_offset`. |
| `timeline_visible_row_count` | `Cell<usize>` | Render-hint write-back: actual visible thread-row count drawn last frame; used by the `↑/↓` scroll handler to bound scrolling. |
| `timeline_thread_name_map` | `HashMap<i64, String>` | Thread-id-to-name map, now written from `ph="M" name="thread_name"` metadata events received alongside timeline batches; used by the Gantt renderer to label thread rows. |
| `timeline_events_filter` | `TimelineFilter` | Current filter selection — `All`, `Ui`, or `Raster`. |

**`TimelineFilter` enum:**

`TimelineFilter` (defined in `fdemon-app/src/session/performance.rs`) controls which thread's events appear in the Timeline Events tab. The three variants cycle `All → Ui → Raster → All` when the user presses `f`. `TimelineFilter::All` is the default.

**Phase 3: `SessionHandle` timeline task fields:**

Three new fields mirror the existing `perf_*` / `alloc_*` / `network_*` pattern:

| Field | Type | Purpose |
|---|---|---|
| `timeline_shutdown_tx` | `Option<Arc<watch::Sender<bool>>>` | Signals the timeline polling task to stop. |
| `timeline_pause_tx` | `Option<Arc<watch::Sender<bool>>>` | Pauses/resumes the timeline polling loop (false = running, true = paused). |
| `timeline_task_handle` | `Option<JoinHandle<()>>` | Join handle for the background timeline polling task. |

**Phase 3: new `Message` variants:**

| Variant | Source | Handler |
|---|---|---|
| `RebuildStatsEventReceived { session_id, payload }` | `forward_vm_events` (`actions/vm_service.rs`) on `Flutter.RebuiltWidgets` extension event | `handler/devtools/performance/rebuild_stats.rs` |
| `ToggleRebuildStats { session_id }` | `R` key binding | `handler/devtools/performance/rebuild_stats.rs` |
| `RebuildStatsExtensionStateChanged { session_id, enabled }` | On-success follow-up from toggle RPC | `handler/devtools/performance/rebuild_stats.rs` |
| `RebuildStatsLocationMapFetched { session_id, location_map }` | One-shot `widgetLocationIdMap` RPC response | `handler/devtools/performance/rebuild_stats.rs` |
| `RebuildStatsToggleFailed { session_id, reason }` | On-failure follow-up from toggle RPC or `widgetLocationIdMap` fetch | `handler/devtools/performance/rebuild_stats.rs` — appends a `Warning` log entry |
| `TimelineEventsBatchReceived { session_id, events, metadata }` | `spawn_timeline_polling` response | `handler/devtools/performance/timeline.rs` |
| `TimelineEventsCycleFilter { session_id }` | `f` key binding on Timeline Events tab | `handler/devtools/performance/timeline.rs` |
| `VmServiceTimelineMonitoringStarted { session_id, shutdown_tx, pause_tx, task_handle }` | `spawn_timeline_polling` startup | `handler/update.rs` — stores handles on `SessionHandle` |

**Phase 3: VM Service additions:**

New constants in `fdemon-daemon/vm_service/extensions/mod.rs::ext`:

| Constant | Value |
|---|---|
| `PROFILE_WIDGET_BUILDS` | `"ext.flutter.profileWidgetBuilds"` |
| `WIDGET_LOCATION_ID_MAP` | `"ext.flutter.inspector.widgetLocationIdMap"` |

New module `fdemon-daemon/vm_service/extensions/performance.rs`:

| Function | Purpose |
|---|---|
| `set_profile_widget_builds(client, isolate_id, enabled)` | Enables/disables widget build profiling; pass `None` to read current state. |
| `get_profile_widget_builds(client, isolate_id)` | Convenience wrapper — calls `set_profile_widget_builds` with `None`. |

New functions in `fdemon-daemon/vm_service/extensions/inspector.rs`:

| Function | Purpose |
|---|---|
| `widget_location_id_map(client, isolate_id)` | Fetches the static `widgetLocationIdMap` from the Flutter inspector; takes a `VmServiceClient`. |
| `widget_location_id_map_handle(handle, isolate_id)` | Identical to `widget_location_id_map` but accepts a `VmRequestHandle`; used from action tasks where only the handle is available. |

New functions in `fdemon-daemon/vm_service/timeline.rs`:

| Function | Purpose |
|---|---|
| `get_vm_timeline_micros(handle)` | Returns the current VM clock in microseconds; used as the chunk-fetch cursor. |
| `fetch_timeline_chunk(handle, since_micros)` | Fetches timeline events since `since_micros`; parses with `fdemon-core::timeline::parse_vm_timeline`. Backward-compatible; preserved for existing consumers. |
| `fetch_timeline_chunk_with_metadata(handle, since_micros)` | Like `fetch_timeline_chunk` but uses `parse_vm_timeline_with_metadata`, returning both events and `Vec<ThreadMetadata>`. Used by `run_one_timeline_fetch_cycle` in Phase 4. |

**Phase 3: `spawn_timeline_polling` task:**

`fdemon-app/src/actions/performance::spawn_timeline_polling` starts a background task that polls `fetch_timeline_chunk` at approximately 1 Hz. The task is gated on `timeline_pause_tx`: `false` = polling active (entered Performance tab), `true` = paused (left Performance tab). The task is shut down via `timeline_shutdown_tx` on session teardown.

`TIMELINE_POLL_MIN_MS` (200 ms) is a compile-time safety floor on the poll interval; it prevents accidental sub-200 ms polling when `poll_interval_ms` is misconfigured. The user-facing default of 1 Hz (1000 ms) is well above this floor.

**Timeline watermark seeding:** before the first poll tick, the task calls `seed_timeline_watermark`, which queries `getVMTimelineMicros` to establish the starting cursor. On failure it retries once after 100 ms; if the retry also fails, it falls back to a wall-clock "now-ish" estimate. This bounds the first fetch extent to milliseconds rather than the full VM-lifetime event buffer.

**Timeline pause and buffer clear on panel leave:** when the user navigates away from the Performance panel (`handle_switch_panel`) or exits DevTools mode (`handle_exit_devtools_mode`), the handler sends `true` on `timeline_pause_tx` to pause polling, and clears `PerformanceState::timeline_tracks`, `timeline_thread_name_map`, and resets `timeline_thread_scroll_offset` to `0`. This ensures the next entry to the Performance panel starts from a clean, current-data state rather than displaying a stale backlog. The same exit path also closes the `Flutter.RebuiltWidgets` panel gate for all sessions.

**Post-fetch watermark update:** after each successful `fetch_timeline_chunk` call, the task re-queries `getVMTimelineMicros` to advance the watermark to the post-fetch VM clock. Using the pre-fetch timestamp plus one would silently drop events whose `ts` falls in the window between the pre-fetch query and the moment the fetch completes. The post-fetch query closes this gap. On failure, the task falls back to `pre_fetch_micros + 1` to maintain forward progress.

**`VmRequestApi` trait (`fdemon-daemon/vm_service/request_api.rs`):** a minimal `pub` trait with two async methods — `request` and `call_extension` — mirroring the signatures on `VmRequestHandle`. `VmRequestHandle` implements it for production use. `spawn_timeline_polling` and the free functions it calls (`get_vm_timeline_micros`, `fetch_timeline_chunk`) are generic over `impl VmRequestApi`, which allows unit tests to substitute a `MockVmRequestApi` without the concrete WebSocket infrastructure. Test-only mock types remain `#[cfg(test)]`-gated. This is the only polling task currently using this trait; higher-level helpers that need `main_isolate_id` call free functions rather than the trait directly.

**Phase 3: `Flutter.RebuiltWidgets` event dispatch:**

`forward_vm_events` in `fdemon-app/src/actions/vm_service.rs` inspects the `extensionKind` field of each Flutter VM extension event. When `extensionKind == "Flutter.RebuiltWidgets"`, the event data is parsed with `fdemon-core::rebuild_stats::parse_rebuilt_widgets_event` and forwarded as `Message::RebuildStatsEventReceived`.

**Phase 3-followup: forwarder panel gate:**

`forward_vm_events` consults `rebuilt_widgets_gate_rx` — a `watch::Receiver<bool>` — before parsing any `Flutter.RebuiltWidgets` event. When the received value is `false` (gate closed), the branch continues immediately without parsing or allocating. The gate is open only when the Performance panel is the active DevTools panel; it is closed on all other panel switches and on DevTools exit. The sender (`rebuilt_widgets_gate_tx: Option<Arc<watch::Sender<bool>>>`) is held on `SessionHandle` and updated by `handle_switch_panel` and `handle_exit_devtools_mode`. This eliminates per-frame allocation and message dispatch at ~60 fps when the user is viewing Inspector, Memory, Network, or normal logs.

`Flutter.RebuiltWidgets` events are sent to the TEA handler via `msg_tx.try_send(...)` rather than `.send().await`. This avoids head-of-line blocking: if the handler is slow, the channel back-pressure drops the current frame rather than stalling the forwarder loop (which would delay `Flutter.Frame` events and error forwarding). A `TrySendError::Full` result is logged at `debug` level; `TrySendError::Closed` exits the loop.

**Phase 3: hot-restart re-enable of `profileWidgetBuilds`:**

When `Message::SessionRestartCompleted` is processed, `fdemon-app/src/handler/update.rs` checks `handle.session.performance.rebuild_stats_enabled`. If `true`, it re-dispatches a `ToggleRebuildStats` action to re-enable `ext.flutter.profileWidgetBuilds` on the new post-restart isolate. This is necessary because hot restart replaces the Dart isolate, clearing all service extensions.

**Phase 3-followup: `RebuildStatsToggleFailed` and rollback flow:**

When the `ToggleProfileWidgetBuilds` RPC fails, or when the subsequent `FetchWidgetLocationIdMap` action fails, the action task emits two messages in sequence: `RebuildStatsExtensionStateChanged { enabled: <actual_state> }` (the rollback) followed by `RebuildStatsToggleFailed { reason }`. The handler for `RebuildStatsToggleFailed` (`handle_toggle_failed` in `handler/devtools/performance/rebuild_stats.rs`) appends a `Warning`-level `LogEntry` to the session log so the user sees the failure without a modal. The companion `RebuildStatsExtensionStateChanged` rollback fires first to keep `rebuild_stats_enabled` consistent with the actual extension state before the warning is displayed.

**Phase 3-followup: `FetchWidgetLocationIdMap` action-layer cleanup:**

The `FetchWidgetLocationIdMap` action in `fdemon-app/src/actions/mod.rs` calls `fdemon_daemon::vm_service::widget_location_id_map_handle` directly and forwards the typed `LocationMap` as `Message::RebuildStatsLocationMapFetched`. The action task is a thin transport wrapper — it resolves the isolate ID, calls the daemon helper, and dispatches the result or a `RebuildStatsToggleFailed` on error. This mirrors the `FetchAllocationProfile` pattern and means no raw JSON parsing remains in `fdemon-app`.

**Phase 3-followup: `auto_enable_rebuild_tracking` wiring:**

`auto_enable_rebuild_tracking` (a `[devtools]` config key) is consulted in the `VmServiceConnected` handler. After the `PerformanceState` reset that runs on every connect, `rebuild_stats_enabled` is always `false`. If `auto_enable_rebuild_tracking == true`, the handler appends a `ToggleProfileWidgetBuilds { enabled: true }` action to `UpdateResult::extra_actions`. The hot-restart re-enable path in `SessionRestartCompleted` is independent — it re-enables only when `rebuild_stats_enabled` was `true` at restart time — and the two paths are mutually exclusive in practice: `VmServiceConnected` fires on first connect and the `SessionRestartCompleted` path fires on subsequent restarts.

**Conditional `RebuildStats` tab visibility and `next_visible` cycle:**

The `RebuildStats` tab in the TUI tab bar (`fdemon-tui/src/widgets/devtools/performance/details/mod.rs`) is shown only when `perf_state.rebuild_stats_enabled == true`. `PerfDetailsTab::next_visible(rebuild_stats_enabled: bool)` (defined in `fdemon-app/src/state.rs`) computes the next visible tab in the cycle, skipping `RebuildStats` when it is disabled: `FrameAnalysis → TimelineEvents → FrameAnalysis`. When enabled, it delegates to the full three-step `next()` cycle. `PerfCycleDetailsTab` uses `next_visible` so the user never lands on the disabled tab by cycling. The `RebuildStats` variant is skipped regardless of which tab the user is currently on when the flag is false — including when the cursor is already on `RebuildStats` (e.g. the user disabled tracking mid-session).

**Phase 3: `details_pane_visible_height` first consumer:**

The `details_pane_visible_height` render-hint `Cell<usize>` field (added to `PerformanceState` in Phase 2, first consumed in Phase 3) is written by the `DetailsPane` renderer and read by the `PgUp`/`PgDn` scroll handlers for both the Rebuild Stats table and the Timeline Events list.

**Phase 3: new `[devtools]` config keys:**

Three new keys are added to the `DevToolsSettings` struct (`fdemon-app/src/config/types.rs`) and loaded from `[devtools]` in `.fdemon/config.toml`:

| Key | Type | Default | Purpose |
|---|---|---|---|
| `auto_enable_rebuild_tracking` | `bool` | `false` | When `true`, `ext.flutter.profileWidgetBuilds` is enabled automatically on VM Service connect. |
| `rebuild_stats_frame_window` | `u32` | `30` | Number of frames retained in the Rebuild Stats ring buffer. |
| `timeline_event_buffer_size` | `usize` | `10000` | Maximum total node count retained across all `timeline_tracks`. Eviction drops the oldest root event globally (by `ts`) until the total is within the cap. Raised from 1000 to 10_000 in Phase 5. |

**Phase 3: new letter shortcuts (architecturally relevant):**

| Key | Context | Effect |
|---|---|---|
| `R` | Performance panel, Details focused on Rebuild Stats tab | Toggles `ext.flutter.profileWidgetBuilds`; emits `Message::ToggleRebuildStats`. Falls through to `HotRestart` in all other contexts (Inspector, Memory, Network, FrameChart, FrameAnalysis tab, TimelineEvents tab). |
| `f` | Performance panel, Details focused on Timeline Events tab | Cycles `TimelineFilter` (`All → Ui → Raster → All`); emits `Message::TimelineEventsCycleFilter`. |

**Phase 3: `fdemon-core` rebuild stats and timeline types:**

`fdemon-core/src/rebuild_stats.rs`:

| Type / Function | Purpose |
|---|---|
| `Location` | Source location for a widget build site (file URI, line, column, name). |
| `LocationMap` | Map from location id (`u32`) to `Location`; supports incremental merge via `merge_parallel_arrays`. |
| `RebuildLocation` | Associates a `Location` with a `build_count` for a single frame snapshot. |
| `RebuildStatsSnapshot` | Per-frame rebuild snapshot: `frame_number`, `start_time_micros`, `Vec<RebuildLocation>`. |
| `RebuildEventPayload` | Raw decoded payload from a `Flutter.RebuiltWidgets` event. |
| `parse_rebuilt_widgets_event` | Parses the JSON payload of a `Flutter.RebuiltWidgets` VM extension event into `RebuildEventPayload`. |

`fdemon-core/src/timeline.rs`:

| Type / Function | Purpose |
|---|---|
| `TimelineThread` | Thread classification — `Ui`, `Raster`, `Other`. |
| `TimelinePhase` | Event phase — `Begin`, `End`, `Complete`, `Instant`, `Other`. |
| `TimelineEvent` | A single VM timeline event: `name`, `category`, `thread`, `tid`, `phase`, `ts`, `dur`, `frame_number`. |
| `parse_vm_timeline` | Parses the JSON response of a `getVMTimeline` call into `Vec<TimelineEvent>`. Backward-compatible; filters `ph="M"` metadata events. |
| `TimelineNode` | A duration-reconstructed event node with `name`, `ts`, `dur`, `phase`, `children`, `category`. Begin/End pairs are reconciled into a single node; children are nested by interval containment within the same `tid`. |
| `TimelineTrack` | Per-thread container: `tid`, `name`, `thread`, `root_events: Vec<TimelineNode>`. Iteration of a `BTreeMap<i64, TimelineTrack>` yields tracks in `tid` ascending order. |
| `ThreadMetadata` | Extracted from `ph="M" name="thread_name"` events: `tid` and human-readable `name` (e.g. `"io.flutter.raster"`). |
| `pair_be_events` | Stack-based B/E pair reconstruction for a single `tid`'s event slice. Unmatched Begin events emit with `dur=None`; mismatched B/E names pop defensively and log at debug. Nesting by interval containment runs after flattening. |
| `build_tracks` | Groups a `Vec<TimelineEvent>` by `tid`, calls `pair_be_events` per group, returns `BTreeMap<i64, TimelineTrack>`. |
| `parse_vm_timeline_with_metadata` | Like `parse_vm_timeline` but also returns `Vec<ThreadMetadata>` extracted from `ph="M"` events. |

**Phase 3-followup: `PROFILE_WIDGET_BUILDS` constant and `I64_MAX_AS_U64` guard:**

`PROFILE_WIDGET_BUILDS` (`"ext.flutter.profileWidgetBuilds"`) is defined in `fdemon-daemon/vm_service/extensions/mod.rs::ext` and used both by `set_profile_widget_builds` in `extensions/performance.rs` and by the `enable_frame_tracking` helper in `timeline.rs`. Using the named constant (rather than a raw string literal) at every call site prevents typo divergence.

`I64_MAX_AS_U64` is a private constant in `fetch_timeline_chunk` (`timeline.rs`) that clamps the `u64` timestamp arguments to `i64::MAX` before casting them to `i64` for the `getVMTimeline` JSON params. This guards against undefined behaviour on pathological values from the VM clock.

**Phase 3-followup: `text_helpers` module (`fdemon-tui`):**

`fdemon-tui/src/widgets/devtools/performance/details/text_helpers.rs` is a `pub(super)` module shared by the sibling tab renderers within `details/`. It provides `truncate_with_ellipsis`, `pad_right`, `pad_left`, and the `PLACEHOLDER_LINE_COUNT` constant. All exports are `pub(super)` — invisible outside the `details` module subtree. Integration tests for `VmRequestApi` polling live in `fdemon-app/src/actions/performance.rs` (inline `#[cfg(test)]` module).

**Phase 4: Timeline Event Tree Model:**

Timeline events are stored per-thread as trees of `TimelineNode` instances rather than a flat ring buffer. `fdemon-core::timeline::pair_be_events` reconstructs Begin/End pairs into duration nodes using a stack-based algorithm, then nests them by interval containment within each `tid`. `PerformanceState::timeline_tracks: BTreeMap<i64, TimelineTrack>` holds the result, with stable thread ordering by `tid` ascending. The polling task calls `fetch_timeline_chunk_with_metadata`, which uses `parse_vm_timeline_with_metadata` to extract `ph="M" name="thread_name"` metadata events alongside the event stream, so `timeline_thread_name_map` is populated with human-readable names like `"io.flutter.raster"`. Incoming batches are merged into existing tracks via `build_tracks` → merge; the per-track buffer cap (`timeline_event_buffer_size`) is enforced by evicting the globally oldest root event (by `ts`) until total node count is within the limit.

**Phase 4: Gantt Timeline Widget (`fdemon-tui`):**

The Timeline Events tab renders as a Gantt chart. The subdirectory `widgets/devtools/performance/details/timeline_events/` contains these modules:

| Module | Role |
|---|---|
| `mod.rs` | Entry point: search bar slot, filter strip, minimap slot, dispatch to Gantt renderer |
| `gantt.rs` | Thread rows with left-column labels, colored event bars across a time canvas, depth-stacked children, time axis, PAUSED indicator |
| `gantt_tests.rs` | External test module for `gantt.rs` (extracted Phase 5 T01 to keep `gantt.rs` manageable) |
| `palette.rs` | Two-color depth-alternating palette per `TimelineThread` (`Ui`=LightBlue/Blue, `Raster`=Blue/DarkGray, `Other`=Magenta/LightMagenta) |
| `viewport.rs` | Pure math helpers: `compute_active_viewport` (3-mode), `zoom_viewport`, `pan_viewport`, `micros_to_column`, `clip_bar` |
| `minimap.rs` | 1-row minimap ribbon with dominant-thread coloring and viewport bracket overlay (Phase 5 T02) |
| `popup.rs` | Modal event details popup with parent-chain breadcrumb (Phase 5 T03) |
| `search.rs` | Search bar widget with match count and hotkey hints (Phase 5 T04) |

Each thread row is `THREAD_ROW_HEIGHT` (6) lines tall, accommodating up to `MAX_DEPTH` (5) depth-stacked child bars. Thread rows are vertically scrollable via `↑/↓` using `timeline_thread_scroll_offset`; the Gantt renderer writes the actual visible row count back to `timeline_visible_row_count` each frame. Thread filtering (`f` key — `All → Ui → Raster → All`) is preserved.

**Phase 4: Immediate Timeline Fetch on Unpause:**

`spawn_timeline_polling` now uses `tokio::select!` mirroring the allocation-polling pattern: when `timeline_pause_rx.changed() → false` fires (Performance panel entered), a helper `run_one_timeline_fetch_cycle` runs immediately before re-entering the 1-Hz tick loop. This eliminates the ~1 s cold-start placeholder window on every Performance-panel entry. The helper returns a `FetchOutcome` enum (`Ok`, `TransientError`, `ChannelClosed`) for testability.

**Phase 4: Frame-Chart Selection and Bar-Height Fixes:**

`compute_visible_range` in `frame_chart/bars.rs` now uses `frame_chart_scroll_offset` as the sole viewport authority — the selected frame is no longer anchored to the right edge. `handle_select_performance_frame` only adjusts `scroll_offset` when the selection moves outside the visible viewport (selection within viewport leaves the offset unchanged). `ms_to_half_blocks` clamps nonzero `ms` values to at least `MIN_BAR_HALF_BLOCKS = 1` half-block, preventing fast frames from disappearing in shallow terminal windows. The selection highlight is a full-column Option-A side-marker overlay (`▏` left-eighth / `▕` right-eighth) spanning every chart row, replacing the previous single-`▔` top-row indicator.

**Phase 5: Three-Mode Viewport State Machine:**

`compute_active_viewport` (in `viewport.rs`) resolves the Gantt viewport in priority order: (1) **manual** — `!follow_latest` returns `(viewport_start_micros, viewport_start_micros + viewport_width_micros)`; (2) **frame-anchored** — `follow_latest && committed_frame_anchor.is_some()` returns `compute_frame_anchored_viewport(frame_anchor_map, frame)` (introduced Phase 4); (3) **live-edge** — fallback returns the latest `TIMELINE_VIEWPORT_MICROS` (5 s) window. Pan (`←`/`→` on TimelineEvents tab, no selection active) and zoom (`+`/`-`) set `timeline_follow_latest = false`, promoting to manual mode; the frame anchor is preserved so `g` (primary) or `End` (TimelineEvents-tab guarded alias) restores the frame-anchored view rather than falling through to live-edge. A "PAUSED" indicator renders in the time-axis row whenever `!follow_latest`. Viewport constants (`TIMELINE_VIEWPORT_MIN_MICROS`, `TIMELINE_VIEWPORT_MAX_MICROS`, `TIMELINE_ZOOM_FACTOR`, `TIMELINE_PAN_FRACTION`) are duplicated in both `fdemon-tui/viewport.rs` and `fdemon-app/timeline.rs` to respect layer boundaries; doc comments require the values to stay in sync.

**Phase 5: Minimap Ribbon:**

A 1-row minimap above the time axis compresses the full event history to canvas width. Each column is colored by the dominant thread in its time slice — the thread whose root events have the largest total duration in that column's range. A `[...]` overlay marks the current viewport position. The minimap walks only depth-0 root events for dominance computation so cost is bounded at `O(columns × root_events_count)`. When `area.height <= MIN_HEIGHT_FOR_MINIMAP`, the minimap slot is dropped gracefully. The minimap is a pure read-only consumer of `PerformanceState`; no new state fields were introduced in T02.

**Phase 5: Selection Cursor:**

`PerformanceState::timeline_selected_event: Option<TimelineEventCursor>` identifies the focused event by `(tid, depth, ts)`. The triple is stable as long as the event survives the ring-buffer eviction policy; when eviction removes the pointed-to event the selection is cleared and a `tracing::debug!` entry is emitted. Arrow keys traverse the per-thread tree: `←`/`→` for previous/next sibling at the same depth (wraps), `↑`/`↓` for parent/first-child or cross-thread navigation. Selection auto-pans the viewport to keep the selected event visible, setting `timeline_follow_latest = false` as a side effect. Pan/zoom `←`/`→` keys are gated: they fire only when `timeline_selected_event.is_none()`; when a selection is active, the same keys move the cursor instead. `Enter` with no selection active selects the first root event of the first visible thread (in `tid` ascending order, filter-respected).

**Phase 5: Details Popup:**

`PerformanceState::timeline_details_popup_open: bool` controls a modal overlay. Pressing `Enter` on a selected event opens the popup, which shows the event's full name, category, thread label, `ts` (µs + human-readable relative offset), `dur`, parent chain breadcrumb (max 4 ancestors with `…` truncation), and direct-children count. The popup uses `widgets/devtools/performance/details/timeline_events/popup.rs` with standard `modal_overlay` chrome. Modal-precedence: while the popup is open, `Esc` closes the popup first; a second `Esc` clears the selection; a third falls through to the existing DevTools-exit behavior.

**Phase 5: Search-and-Jump:**

`/` opens a search input on the Timeline Events tab, setting `timeline_search_input_active = true` and `timeline_search_query = Some("")`. Typed characters append to the query; matches are highlighted in real-time (BOLD + UNDERLINED on matching bars). `Enter` commits the query (`search_input_active = false`), arming `n`/`N` for next/previous match cycling. `n`/`N` pan the viewport to center on the next/previous match and update `timeline_selected_event` to the matched cursor; the current match bar receives an additional REVERSED modifier. `Esc` while input is active cancels, setting `timeline_search_query = None`; the search bar disappears. Search is case-insensitive substring matching; an empty committed query matches nothing. `n` falls through to the Network panel handler when `timeline_search_query.is_none()`, preserving the global `n` → Network shortcut. The search state fields (`timeline_search_query`, `timeline_search_input_active`, `timeline_search_match_cursor`) are cleared by both `handle_exit_devtools_mode` and `handle_switch_panel` to prevent stale queries surviving panel re-entry.

**Phase 5: new `PerformanceState` fields:**

| Field | Type | Default | Purpose |
|---|---|---|---|
| `timeline_viewport_start_micros` | `u64` | `0` | Manual viewport start; honored only when `timeline_follow_latest == false`. |
| `timeline_viewport_width_micros` | `u64` | `5_000_000` | Viewport width in microseconds (default 5 s). Bounded by `TIMELINE_VIEWPORT_MIN_MICROS` (100 ms) to `TIMELINE_VIEWPORT_MAX_MICROS` (60 s). |
| `timeline_follow_latest` | `bool` | `true` | When `true`, `compute_active_viewport` uses frame-anchored or live-edge mode; when `false`, uses manual window. |
| `timeline_selected_event` | `Option<TimelineEventCursor>` | `None` | Currently focused event identified by `(tid, depth, ts)`. |
| `timeline_details_popup_open` | `bool` | `false` | Whether the event details modal is open. |
| `timeline_search_query` | `Option<String>` | `None` | Active search query; `None` = search closed; `Some("")` = input open but empty. |
| `timeline_search_input_active` | `bool` | `false` | `true` while user is typing in the search input (before `Enter`/`Esc`). |
| `timeline_search_match_cursor` | `usize` | `0` | Current match index for `n`/`N` navigation; reset to `0` on query change. |

**Phase 5: new `Message` variants:**

| Variant | Source | Handler |
|---|---|---|
| `TimelineZoomIn { session_id }` | `+` key | `handler/devtools/performance/timeline.rs::handle_zoom_in` |
| `TimelineZoomOut { session_id }` | `-` key | `handler/devtools/performance/timeline.rs::handle_zoom_out` |
| `TimelinePanLeft { session_id }` | `←` key (no selection) | `handler/devtools/performance/timeline.rs::handle_pan_left` |
| `TimelinePanRight { session_id }` | `→` key (no selection) | `handler/devtools/performance/timeline.rs::handle_pan_right` |
| `TimelineFollowLatest { session_id }` | `g` / `End` key | `handler/devtools/performance/timeline.rs::handle_follow_latest` |
| `TimelineSelectFirstVisible { session_id }` | `Enter` (no selection) | `handler/devtools/performance/timeline.rs::handle_select_first_visible` |
| `TimelineMoveSelection { session_id, direction }` | Arrow keys (selection active) | `handler/devtools/performance/timeline.rs::handle_move_selection` |
| `TimelineOpenPopup { session_id }` | `Enter` (selection active) | `handler/devtools/performance/timeline.rs::handle_open_popup` |
| `TimelineClosePopup { session_id }` | `Esc` (popup open) | `handler/devtools/performance/timeline.rs::handle_close_popup` |
| `TimelineClearSelection { session_id }` | `Esc` (popup closed, selection active) | `handler/devtools/performance/timeline.rs::handle_clear_selection` |
| `TimelineSelectAt { session_id, cursor }` | Mouse click on event bar | `handler/devtools/performance/timeline.rs::handle_select_at` |
| `TimelineSearchOpen { session_id }` | `/` key | `handler/devtools/performance/timeline.rs::handle_search_open` |
| `TimelineSearchInputChar { session_id, ch }` | Char key (input active) | `handler/devtools/performance/timeline.rs::handle_search_input_char` |
| `TimelineSearchInputBackspace { session_id }` | Backspace (input active) | `handler/devtools/performance/timeline.rs::handle_search_input_backspace` |
| `TimelineSearchInputCommit { session_id }` | `Enter` (input active) | `handler/devtools/performance/timeline.rs::handle_search_input_commit` |
| `TimelineSearchInputCancel { session_id }` | `Esc` (input active) | `handler/devtools/performance/timeline.rs::handle_search_input_cancel` |
| `TimelineSearchNextMatch { session_id }` | `n` (query committed) | `handler/devtools/performance/timeline.rs::handle_next_match` |
| `TimelineSearchPrevMatch { session_id }` | `N` (query committed) | `handler/devtools/performance/timeline.rs::handle_prev_match` |

### Memory Panel Interactivity

The Memory panel shows the memory usage time-series chart and the class allocation table. Focus, scrolling, and selection state are tracked on `MemoryState` (in `fdemon-app/src/session/memory.rs`). The top-level widget is `MemoryPanel` (`fdemon-tui/src/widgets/devtools/memory/mod.rs`).

**Section focus (`MemorySection` enum):**

`MemorySection` has two variants — `Chart` (the memory usage time-series chart, default) and `AllocationList` (the class allocation table). `Tab` and `Shift+Tab` cycle between them. Section-specific key and scroll events are gated on the currently focused section.

**Scroll-offset model (live-edge drift):**

- `memory_chart_scroll_offset` — how many samples the memory chart has been scrolled back from the live edge. `0` means the live edge is visible.
- `alloc_table_scroll_offset` — row scroll offset for the allocation table (rows scrolled past the top).
- `alloc_table_selected_row` — index of the selected row in the allocation table, if any.

**Render-hint `Cell<usize>` fields:**

Two fields on `MemoryState` use `Cell<usize>` interior mutability:

| Field | Purpose |
|---|---|
| `memory_chart_visible_width` | Columns available in the memory chart area; used by the scroll handler to clamp scroll offset. |
| `alloc_table_visible_height` | Rows visible in the allocation table; used by `PgUp`/`PgDn` to page by the correct amount. |

Both default to `0` ("not yet rendered — use fallback"). These are the same approved TEA exception class as the existing render-hint cells on `PerformanceState`, `MouseRegions`, and `TagFilterUiState`. No new exception class is introduced — these are renames of the fields that previously lived on `PerformanceState`.

**Buffer sizes:**

| Constant | Value | Coverage |
|---|---|---|
| `DEFAULT_MEMORY_HISTORY_SIZE` | 60 | 2 minutes at 2 s poll interval |
| `DEFAULT_GC_HISTORY_SIZE` | 50 | Major GC events only (Scavenge events filtered) |
| `DEFAULT_MEMORY_SAMPLE_SIZE` | 120 | 60 seconds at 500 ms poll interval |

**Allocation sort (`AllocationSortColumn` enum):**

`AllocationSortColumn` has two variants — `BySize` (total allocated bytes, descending, default) and `ByInstances` (total instance count, descending). This enum was relocated from `session/performance.rs` to `session/memory.rs` as part of the Phase 1 split.

---

## DAP Server Subsystem

The DAP server enables IDE debuggers (VS Code, Zed, Neovim, Helix) to attach to
Flutter sessions managed by fdemon via the Debug Adapter Protocol.

### Architecture Overview

```
┌──────────────────────────────────────────────────────────────┐
│                      IDE (DAP client)                        │
│              VS Code / Zed / Neovim / Helix                  │
└────────────────────────┬─────────────────────────────────────┘
                         │ TCP (DAP wire protocol)
                         ▼
┌──────────────────────────────────────────────────────────────┐
│                    fdemon-dap crate                          │
│  ┌────────────────┐  ┌──────────────────────────────────┐   │
│  │   DapServer    │  │         DapClientSession         │   │
│  │ (TCP listener) │──│  (per-connection state machine)  │   │
│  └────────────────┘  └──────────────┬───────────────────┘   │
│                                     │                        │
│                          ┌──────────▼──────────┐            │
│                          │      DapAdapter      │            │
│                          │  (protocol handler)  │            │
│                          └──────────┬───────────┘            │
│                                     │ DebugBackend trait     │
└─────────────────────────────────────┼──────────────────────┘
                                      │
┌─────────────────────────────────────┼──────────────────────┐
│               fdemon-app crate      │                       │
│                                     ▼                       │
│                          ┌──────────────────────┐          │
│                          │  VmServiceBackend    │          │
│                          │ (DebugBackend impl)  │          │
│                          └──────────┬───────────┘          │
│                                     │                       │
│          ┌──────────────────────────┼──────────┐           │
│          ▼                          ▼           ▼           │
│  dap_debug_senders          TEA Engine    VmRequestHandle  │
│  (DebugEvent channel)      (hot reload)   (VM Service RPC) │
└──────────────────────────────────────────────────────────┘
```

### Debug Event Flow

VM Service debug events (breakpoint hit, resume, isolate created) are translated
into DAP events and forwarded to connected IDE clients:

```
Dart VM Service
    │
    ├── "Debug" stream events ──────────────────────┐
    │   (PauseBreakpoint, Resume, PauseException)   │
    │                                               ▼
    │                                  actions/vm_service.rs
    │                                  (VM event forwarding loop)
    │                                               │
    │                                               ▼
    │                                  Message::VmServiceDebugEvent
    │                                               │
    │                                               ▼
    │                                  handler/devtools/debug.rs
    │                                  handle_debug_event()
    │                                               │
    │                               ┌───────────────┴─────────────────┐
    │                               ▼                                 ▼
    │                    Mutate per-session DebugState        Translate to DapDebugEvent
    │                    (paused/resumed/isolate state)       (Paused/Resumed/ThreadExited)
    │                                                                  │
    └── "Isolate" stream events                                        ▼
        (IsolateStart, IsolateExit)                     dap_debug_senders registry
                │                                       (one mpsc::Sender per DAP client)
                ▼                                                       │
        handler/devtools/debug.rs                                       ▼
        handle_isolate_event()                             DapAdapter.process_debug_event()
                                                                        │
                                                                        ▼
                                                           IDE receives stopped/continued/
                                                           thread DAP events
```

### Channel Architecture: `dap_debug_senders`

The `dap_debug_senders` registry is the bridge between the TEA message loop and
the per-connection DAP adapters:

```
AppState
└── dap_debug_senders: Arc<Mutex<Vec<mpsc::Sender<DebugEvent>>>>
          │
          │ (one entry per connected DAP client)
          │
          ├── Sender → DapClientSession 1 (IDE window 1)
          ├── Sender → DapClientSession 2 (IDE window 2)
          └── ...
```

- When a DAP client attaches, the Engine creates an `mpsc` channel and registers
  the `Sender` in `dap_debug_senders`.
- The TEA handler calls `try_send` on each sender when a debug event arrives.
- Stale senders (where the DAP client disconnected) are pruned automatically:
  `try_send` returns `Err` for a closed channel, and the handler uses `retain`
  to remove them.

### Breakpoint State Model

Each `DapAdapter` instance holds a `BreakpointState` that tracks the mapping
between DAP breakpoint IDs (integers) and VM Service breakpoint IDs (strings):

```
setBreakpoints (IDE request)
    │
    ▼
BreakpointState
├── by_dap_id: HashMap<i64, BreakpointEntry>
│   └── BreakpointEntry {
│       dap_id, vm_id, uri, line, column, verified,
│       condition, hit_condition, hit_count, log_message
│   }
└── vm_id_to_dap_id: HashMap<String, i64>

When VM emits PauseBreakpoint (vm_id):
  1. Look up dap_id via vm_id_to_dap_id
  2. Increment hit_count
  3. Evaluate hit_condition (if any) — cheap, no VM RPC
  4. Evaluate condition via evaluateInFrame (if any)
  5. If logpoint: interpolate {expression} and emit output event, auto-resume
  6. If all conditions pass: emit stopped event to IDE
```

### Multi-Session Thread ID Namespacing

Each Flutter session is assigned a dedicated thread ID range so that isolates
from different sessions cannot collide:

```
Session index  │  Thread ID range  │  Formula
───────────────┼───────────────────┼─────────────────────────────
0              │  1000–1999        │  (0+1) × 1000 = 1000
1              │  2000–2999        │  (1+1) × 1000 = 2000
2              │  3000–3999        │  (2+1) × 1000 = 3000
…              │  …                │  …
8              │  9000–9999        │  (8+1) × 1000 = 9000
```

Given a thread ID, the session index is recovered as: `(thread_id / 1000) - 1`.
The `ThreadMap` inside each session converts between Dart isolate IDs (strings
like `"isolates/12345"`) and namespaced DAP thread IDs (integers).

### Coordinated Pause: Auto-Reload Suppression

When the Dart VM pauses an isolate (breakpoint, exception, step), file-watcher
triggered hot reloads are suppressed to avoid invalidating the paused stack
frame:

```
PauseBreakpoint event received
    │
    ▼
handle_debug_event()
    ├── Update DebugState (paused = true)
    ├── Forward DapDebugEvent::Paused to IDE clients
    └── Emit Message::SuspendFileWatcher (follow-up)
            │
            ▼
    AppState.file_watcher_suspended = true
    (file changes queued in pending_watcher_changes)

Resume event received (or DAP client disconnects)
    │
    ▼
    AppState.file_watcher_suspended = false
    If pending_watcher_changes > 0: trigger single hot reload
```

### Custom DAP Events

On successful `attach`, fdemon emits three custom events to the IDE:

```
dart.debuggerUris
  body: { "vmServiceUri": "ws://127.0.0.1:PORT/..." }
  → Allows IDE to connect supplementary tooling (Dart DevTools) to the same
    VM Service connection

flutter.appStart
  body: { "deviceId": "...", "mode": "debug", "supportsRestart": true }
  → Signals session metadata to the IDE debugger extension

flutter.appStarted
  body: {}
  → Emitted when the VM signals the app is fully started (IsolateRunnable /
    AppStarted VM event)
```

Phase 6 adds further custom events:

```
dart.hotReloadComplete
  body: {}
  → Emitted after a successful hot reload completes (sourced from EngineEvent)

dart.hotRestartComplete
  body: {}
  → Emitted after a successful hot restart completes (sourced from EngineEvent)

dart.serviceExtensionAdded
  body: { "extensionRPC": "ext.flutter.xxx", "isolateId": "..." }
  → Forwarded from the VM Service ServiceExtensionAdded stream event; lets
    the IDE discover available extension methods dynamically

progressStart / progressEnd  (standard DAP events)
  → Emitted around hot reload and hot restart when the connected client
    declared supportsProgressReporting: true in its initialize arguments.
    The adapter generates monotonically increasing progress IDs to pair events.
```

### DAP Request Inventory

The following DAP requests are handled by `DapAdapter::handle_request`. Requests
introduced or completed in Phase 6 are marked *(Phase 6)*.

| Request | Purpose |
|---------|---------|
| `attach` | Bind to an active Flutter session, discover isolates, emit session-start custom events |
| `disconnect` | Detach from the debug session; optionally terminate the Flutter app |
| `threads` | Return all known Dart isolates as DAP thread objects |
| `setBreakpoints` | Sync desired breakpoints to the VM; supports conditions, hit-conditions, logpoints |
| `setExceptionBreakpoints` | Configure `None` / `Unhandled` / `All` pause-on-exception mode |
| `continue` | Resume a paused isolate |
| `next` | Step over one statement |
| `stepIn` | Step into a call |
| `stepOut` | Step out of the current frame |
| `pause` | Interrupt a running isolate |
| `stackTrace` | Return call frames for a paused isolate; marks async suspension boundaries |
| `scopes` | Return variable scopes for a frame — Locals, Globals, Exceptions |
| `variables` | Expand a variable reference; handles Record, WeakReference, Sentinel, Set, truncated strings |
| `evaluate` | Evaluate an expression in a frame or target context; supports `$_threadException` |
| `source` | Serve SDK / unresolvable source text via VM Service `getObject` |
| `hotReload` | Trigger Flutter hot reload via the TEA pipeline |
| `hotRestart` | Trigger Flutter hot restart via the TEA pipeline |
| `restart` | *(Phase 6)* Maps to `hot_restart()` — the standard DAP restart request |
| `restartFrame` | *(Phase 6)* Rewind execution to a previous stack frame using `StepMode::Rewind`; guarded against async suspension boundaries |
| `loadedSources` | *(Phase 6)* Return all Dart script URIs currently loaded in the isolate |
| `callService` | *(Phase 6)* Forward an arbitrary VM Service RPC; used by IDE extensions for custom DevTools integration |
| `exceptionInfo` | *(Phase 6)* Return full exception details (type, message, stack trace) for the thread stopped at an exception |
| `updateDebugOptions` | *(Phase 6)* Toggle SDK library and external package library debuggability; applies `setLibraryDebuggable` to all known libraries |
| `breakpointLocations` | *(Phase 6)* Return valid breakpoint positions within a source range using `getSourceReport(PossibleBreakpoints)` |
| `completions` | *(Phase 6)* Return auto-complete suggestions (local variables + Dart keywords) for the debug console |

### Variable System (Phase 6 Overhaul)

The variable system — implemented in `adapter/variables.rs` — was significantly
expanded in Phase 6. Key design decisions:

**Variable type rendering:**

| Dart VM type | Display strategy |
|--------------|-----------------|
| `Instance` (PlainInstance) | Class name; optional `toString()` suffix; fields + evaluated getters |
| `Record` | `(field1, field2, ...)` positional + named fields |
| `WeakReference` | Shows `target` field; labeled `WeakReference<T>` |
| `Sentinel` | Displays sentinel reason (`expired`, `collected`, etc.) directly as value |
| `String` (truncated) | Shows truncated preview; expands via `getObject` with offset/count |
| `List` / `Set` | Index-keyed children; page-based expansion; Set items are fetched via `getObject` |
| `Map` | Key-value pairs from association list |

**`evaluateName` construction:**

Each expanded variable is assigned an `evaluateName` expression — a
syntactically valid Dart expression that can re-evaluate to the same value. The
`evaluate_name_map` on `DapAdapter` (keyed by variable reference) stores the
parent expression so that child expressions can be composed:

- Struct field: `parent.fieldName`
- List element: `parent[index]`
- Map value: `parent[keyExpr]`
- Getter: same as the field expression

**Getter evaluation:**

When `evaluate_getters_in_debug_views` is `true` (default), getter methods on
`PlainInstance` objects are eagerly evaluated with a 1-second per-getter timeout.
Getter results appear with `presentationHint.attributes: ["hasSideEffects"]`.
When `false`, getters appear as lazy nodes the user must explicitly expand.

**`toString()` display enrichment:**

When `evaluate_to_string_in_debug_views` is `true` (default), a `toString()`
call is issued for each `PlainInstance`, `RegExp`, and `StackTrace` variable. If
the result is not the default `"Instance of 'ClassName'"` pattern, it is appended
to the display value: `"MyClass (custom repr)"`.

**Globals scope:**

A `Globals` scope is conditionally added to `scopes` for frames in the root
library. The adapter calls `get_isolate()` to retrieve `rootLib`, then lists all
top-level variables from that library object.

**Exception scope:**

A `Exceptions` scope is added when the isolate paused at a `PauseException`
event. The adapter stores the `InstanceRef` in `exception_refs` (keyed by DAP
thread ID). The `exceptionInfo` request uses the same stored ref to serve full
exception details. Both the scope and the stored ref are cleared on resume.

**Safety cap:**

`MAX_VARIABLE_REFS` (10,000) limits the total number of variable references that
can be allocated in a single stop cycle. Expansion requests beyond this cap return
an error response to prevent unbounded memory growth.

### DapAdapter State Fields (Phase 6 Additions)

New per-session state added to `DapAdapter` in Phase 6:

| Field | Type | Purpose |
|-------|------|---------|
| `exception_refs` | `HashMap<i64, ExceptionRef>` | Stores the exception `InstanceRef` for each thread paused at a `PauseException`; cleared on resume |
| `evaluate_name_map` | `HashMap<i64, String>` | Maps variable refs to their evaluatable Dart expressions; cleared on resume |
| `evaluate_getters_in_debug_views` | `bool` | Eagerly evaluate getters on expand (default: `true`); set from `attach` args |
| `evaluate_to_string_in_debug_views` | `bool` | Append `toString()` result to display value (default: `true`); set from `attach` args |
| `first_async_marker_index` | `Option<i32>` | Frame index of the first `AsyncSuspensionMarker`; used to guard `restartFrame` against async boundaries |
| `debug_sdk_libraries` | `bool` | Allow stepping into Dart SDK libraries (default: `false`) |
| `debug_external_package_libraries` | `bool` | Allow stepping into external package libraries (default: `false`) |
| `app_package_name` | `String` | The app's own package name; distinguishes app code from external packages |
| `client_supports_progress` | `bool` | Set from `initialize` args; enables `progressStart`/`progressEnd` events |
| `next_progress_id` | `u64` | Monotonic counter for progress event ID generation |

### `DebugBackend` Trait

`fdemon-dap` defines the `DebugBackend` trait so it does not depend on
`fdemon-daemon` or `fdemon-app`. The concrete implementation,
`VmServiceBackend`, lives in `fdemon-app/src/handler/dap_backend.rs`:

```
fdemon-dap (defines trait)              fdemon-app (implements trait)
┌───────────────────────────┐          ┌──────────────────────────┐
│ pub trait DebugBackend {  │          │ pub struct VmServiceBackend {
│   pause(isolate_id)       │◄─────────│   handle: VmRequestHandle │
│   resume(isolate_id, step,│          │   msg_tx: mpsc::Sender     │
│          frame_index)     │          │   ws_uri: Option<String>   │
│   add_breakpoint(...)     │          │   device_id: Option<String>│
│   evaluate_in_frame(...)  │          │   build_mode: String       │
│   hot_reload()            │          │ }                          │
│   hot_restart()           │          │                            │
│   ws_uri()                │          │ // hot_reload / hot_restart│
│   get_source(...)         │          │ // send Message::HotReload │
│   get_isolate(isolate_id) │          │ // into TEA pipeline       │
│   call_service(method,..) │          │                            │
│   set_library_debuggable()│          │                            │
│   get_source_report(...)  │          │                            │
│   ...                     │          │                            │
│ }                         │          │                            │
└───────────────────────────┘          └──────────────────────────┘
```

**Phase 6 additions to `DebugBackend`:**

| Method | Purpose |
|--------|---------|
| `get_isolate(isolate_id)` | Get full isolate object — `rootLib`, `libraries[]`, `pauseEvent`. Used for globals scope enumeration and `updateDebugOptions`. |
| `call_service(method, params)` | Forward arbitrary VM Service RPC calls. Used by the `callService` custom DAP request to expose extension methods without dedicated trait methods. |
| `set_library_debuggable(isolate_id, library_id, is_debuggable)` | Call `setLibraryDebuggable` VM RPC — controls SDK/external library stepping. |
| `get_source_report(isolate_id, script_id, kinds, ...)` | Call `getSourceReport` VM RPC for `PossibleBreakpoints` ranges. Used by `breakpointLocations`. |
| `resume(isolate_id, step, frame_index)` | Extended signature — `frame_index` carries the target frame for `StepMode::Rewind` (`restartFrame`). |

`hot_reload()` and `hot_restart()` on `VmServiceBackend` send
`Message::HotReload` / `Message::HotRestart` into the TEA pipeline rather than
calling VM Service RPCs directly. This ensures reload lifecycle, phase tracking,
and EngineEvent broadcasting all work consistently whether reload is triggered
from the TUI, file watcher, or IDE.

All `DebugBackend` calls in the adapter are wrapped with a `REQUEST_TIMEOUT`
(10 seconds) so that a stalled VM Service connection does not block the DAP
session indefinitely.

---

## Native Log Capture Subsystem

Flutter apps on Android and iOS/macOS emit native platform logs (e.g., Go plugin logs, OkHttp network logs) that do not appear on Flutter's stdout/stderr pipe. The native log capture subsystem bridges these platform-specific log streams into the fdemon log view.

### Architecture

```
FlutterProcess starts
    │
    ▼
fdemon-daemon: create_native_log_capture(platform, …)
    │
    ├── "android" ──► AndroidLogCapture
    │                 spawns: adb logcat --pid <pid>
    │
    ├── "macos"   ──► MacOsLogCapture
    │                 spawns: log stream --process <name>
    │
    └── "ios"     ──► IosLogCapture
                      ├── is_simulator=true → xcrun simctl spawn <udid> log stream
                      └── is_simulator=false → idevicesyslog -u <udid> -p <process>
```

Each backend implements `NativeLogCapture::spawn()` which returns a `NativeLogHandle` with:
- `event_rx`: `mpsc::Receiver<NativeLogEvent>` — parsed log events
- `shutdown_tx`: `watch::Sender<bool>` — graceful stop signal
- `task_handle`: `JoinHandle<()>` — background task (abortable as fallback)

### Tag Filtering

All native log events include a `tag` field (e.g., `"GoLog"`, `"OkHttp"`). Per-session tag state is tracked in `NativeTagState` (in `fdemon-app/session/native_tags.rs`):

- Tags are discovered as events arrive and added to `discovered_tags` (a `BTreeMap<String, usize>` tracking count per tag)
- Users can hide individual tags via the tag filter overlay (press `T` in normal mode)
- Hidden tags are stored in `hidden_tags` (`BTreeSet<String>`)
- Filtering is applied at the handler level: entries for hidden tags are not added to the session log buffer
- Un-hiding a tag only applies to future entries (consistent with `LogSourceFilter` behaviour)

### Per-Tag Configuration

Individual tags can be configured in `.fdemon/config.toml` under `[native_logs.tags.<TagName>]`:

```toml
[native_logs.tags.GoLog]
min_level = "debug"   # per-tag minimum level override

[native_logs.tags.OkHttp]
min_level = "info"
```

### Tool Dependencies

| Tool | Platform | Purpose | Availability |
|------|----------|---------|--------------|
| `adb` | Android | logcat log capture | Required for Android native logs |
| `log` | macOS | unified log stream capture | Required for macOS native logs |
| `xcrun simctl` | macOS (iOS sim) | iOS simulator log stream | Requires Xcode CLI tools |
| `idevicesyslog` | macOS (iOS phy) | Physical iOS device syslog relay | Optional; part of `libimobiledevice`. Graceful degradation if absent. |

### Custom Log Sources

Users can define arbitrary log source processes via `[[native_logs.custom_sources]]` configuration. Each custom source implements the same `NativeLogCapture` trait as platform backends.

#### Architecture

```
                     NativeLogCapture trait
┌──────────────────────────────────────────────────────────────────┐
│ AndroidLogCapture │ MacOsLogCapture │ IosLogCapture │ CustomLogCapture │
│ (adb logcat)      │ (log stream)    │ (xcrun simctl/│ (user-defined    │
│                   │                 │  idevicesyslog)│  command)        │
└──────────────────────────────────────────────────────────────────┘
          │                  │               │               │
          └──────────────────┴───────────────┴───────────────┘
                                     │
                              NativeLogEvent
                                     │
                         ┌───────────┴───────────┐
                         │   Format Parser        │
                         │   (formats.rs)         │
                         │   Raw│Json│Logcat│Syslog│
                         └───────────────────────┘
                                     │
                         Message::NativeLog
                                     │
                         handler::update()
                                     │
                         NativeTagState + log buffer
```

`CustomLogCapture` is separate from `create_native_log_capture()` (which dispatches by platform string). Multiple custom sources can be active concurrently within a single session.

**Key design decisions:**

- **No shell expansion**: Commands are spawned directly via `tokio::process::Command::new` with explicit args — never `sh -c`. This avoids injection risks.
- **No auto-restart**: If the process exits, a warning is logged and the capture stops. Users must fix their command configuration.
- **stderr not parsed**: stderr is piped to avoid orphaned pipe errors but its output is not forwarded as log events.
- **Tag filtering**: Reuses `should_include_tag()` from `native_logs/mod.rs` with the per-source `include_tags`/`exclude_tags` lists.

#### Format Parser Dispatch (`native_logs/formats.rs`)

The `formats` module provides pluggable output parsing for custom sources via the `parse_line()` dispatch function:

| Format | `OutputFormat` variant | Parser | Behavior |
|--------|------------------------|--------|----------|
| `raw` | `OutputFormat::Raw` | `parse_raw()` | Each non-empty line → `NativeLogEvent` (Info level, tag = source name) |
| `json` | `OutputFormat::Json` | `parse_json()` | JSON objects with flexible field aliases: message/msg/text, tag/source/logger, level/severity/priority, timestamp/time/ts |
| `logcat-threadtime` | `OutputFormat::LogcatThreadtime` | delegates to `android::parse_threadtime_line()` + `android::logcat_line_to_event()` | Android logcat threadtime format |
| `syslog` | `OutputFormat::Syslog` | delegates to `macos::parse_syslog_line()` + `macos::syslog_line_to_event()` | macOS/iOS unified logging compact format (macOS-only; returns `None` on other platforms) |

Custom sources integrate with the existing pipeline identically to platform backends:
- Events flow through `NativeLogEvent` → `Message::NativeLog` → handler path
- Tags are tracked in `NativeTagState` and appear in the tag filter overlay (`T` key)
- `should_include_tag()` filtering applies identically to platform backends
- `min_level` filtering uses the same `effective_min_level()` logic

#### Custom Source Lifecycle Messages

Two `Message` variants manage custom source lifecycle:

| Message | When sent | Purpose |
|---------|-----------|---------|
| `CustomSourceStarted { session_id, name, shutdown_tx, task_handle }` | After `CustomLogCapture::spawn()` succeeds | TEA handler stores `shutdown_tx` and `task_handle` in `SessionHandle::custom_source_handles` |
| `CustomSourceStopped { session_id, name }` | When the source's event channel closes (process exited) | TEA handler removes the named handle from `custom_source_handles` |

#### Shared Custom Sources (`shared = true`)

Custom sources with `shared = true` are spawned once for the entire project and broadcast their logs to every active session. The TEA handler stores shared handles in `AppState.shared_source_handles` (keyed by name) rather than in per-session state.

```
Shared Custom Sources (shared = true):

┌─────────────────────────────────────────────┐
│ AppState.shared_source_handles              │
│   - "backend" (shutdown_tx, task_handle)    │
└───────────────────┬─────────────────────────┘
                    │ Message::SharedSourceLog
                    ▼
┌─────────────────────────────────────────────┐
│ TEA Handler: broadcast to all sessions      │
│   session_manager.iter_mut()                │
│     → per-session tag filter                │
│     → queue_log()                           │
└─────────────────────────────────────────────┘
```

Contrast with per-session sources, where each session manages its own process lifecycle:

```
Per-Session Custom Sources (shared = false, default):

┌─────────────────────────────────────────────┐
│ SessionHandle.custom_source_handles         │
│   - "worker" (shutdown_tx, task_handle)     │
└───────────────────┬─────────────────────────┘
                    │ Message::NativeLog { session_id }
                    ▼
┌─────────────────────────────────────────────┐
│ TEA Handler: route to specific session      │
│   session_manager.get_mut(session_id)       │
│     → tag filter → queue_log()              │
└─────────────────────────────────────────────┘
```

Shared sources can be started either as pre-app sources (`start_before_app = true`) or as post-app sources (`start_before_app = false`). They are shut down during `AppState::shutdown_shared_sources()` when fdemon exits — after all per-session sources have been stopped.

#### Pre-App Custom Source Flow

Custom sources with `start_before_app = true` gate the Flutter app launch behind a readiness check. The flow diverges from normal session launch at `handle_launch()`:

```
handle_launch()
  → IF has pre-app sources:
      UpdateAction::SpawnPreAppSources
        → spawn pre-app CustomLogCapture processes
        → run readiness checks concurrently (HTTP, TCP, command, stdout, delay)
        → on ready: Message::PreAppSourcesReady
          → UpdateAction::SpawnSession (normal flow continues)
        → on timeout: proceed with warning
  → ELSE:
      UpdateAction::SpawnSession (unchanged)
```

**Readiness check types:**

| Type | Mechanism | Ready when |
|------|-----------|------------|
| `http` | Polls a URL via GET | 2xx HTTP response received |
| `tcp` | Attempts TCP connection to host:port | Connection succeeds |
| `command` | Runs an external command | Exit code 0 |
| `stdout` | Watches the process's stdout | Line matches a regex pattern |
| `delay` | Waits a fixed duration | Duration elapses |

All checks run concurrently. Each has an independent `timeout_s` (default: 30 s). On timeout, the Flutter launch gate lifts anyway with a warning in the log view — the custom source process continues running.

---

## Data Flow

### Startup Sequence

```
1. main.rs: Parse CLI args
2. main.rs: Check if path is runnable Flutter project
3. main.rs: If not, discover projects in subdirectories
4. main.rs: If multiple, show project selector
5. app::run_with_project(): Initialize logging
6. tui::run_with_project(): Initialize terminal
7. tui::run_with_project(): Load settings (config.toml + launch.toml + settings.local.toml)
8. tui::run_with_project(): Spawn background startup tasks (fire-and-forget, non-blocking):
   - spawn_tool_availability_check — detect adb, xcrun simctl, idevicesyslog
   - spawn_bootable_device_discovery — list iOS simulators and Android AVDs
   - spawn_version_check — query GitHub releases API (or serve from on-disk cache); sends Message::NewVersionAvailable if a newer release exists. The handler drops this message silently if ui_mode has already transitioned away from Startup/NewSessionDialog (late-arrival gate).
9. tui::run_with_project(): Flutter SDK resolution — if no SDK resolves, opens
   UiMode::InstallWizard and emits UpdateAction::RunToolchainPreflight (background task).
   InstallWizard can also be opened at any time via the `I` key from Normal mode.
   Phase 5: after a managed Flutter install completes and the post-install preflight
   shows Flutter live, the wizard auto-closes and dispatches DiscoverDevices, handing
   control back to the normal session-launch flow. Both the auto-close path and the
   manual-close path (Esc with a live SDK) delegate to the same shared helper, which
   sets UiMode::Startup (not Normal) before dispatching DiscoverDevices so that the
   subsequent DevicesDiscovered message populates the new-session dialog's target
   selector. Manual close (Esc) with a live SDK triggers the same handback path.
10. tui::run_with_project(): Auto-launch gate — fires when launch.toml has auto_start=true,
    OR when [behavior] auto_launch=true AND a valid last_device is cached.
    Otherwise: show New Session dialog. (See docs/CONFIGURATION.md for the full priority table.)
11. tui::run_with_project(): Spawn Flutter process (if auto-launch fired)
12. tui::run_loop(): Enter main event loop
```

### Hot Reload Flow

```
1. User presses 'r' OR FileWatcher detects change
2. Message::HotReload sent to channel
3. handler::update() processes message:
   - Validates app_id exists
   - Sets phase to Reloading
   - Returns UpdateAction::SpawnTask(Task::Reload)
4. Event loop spawns reload task
5. CommandSender sends app.restart JSON-RPC
6. Flutter process performs reload
7. DaemonEvent::Message(AppProgress{finished:true}) received
8. handler::update() sets phase back to Running
9. tui::render() shows updated status
```

### Session Launch Lifecycle

Each new session progresses through a fixed phase sequence driven by daemon events:

```
Initializing  (session created, no spawn work yet)
    │
    ▼
Preparing     pre-app native-log sources with start_before_app=true are polling
              their ready_check; Flutter process not yet spawned.
              Exits when Message::PreAppSourcesReady is received (or immediately
              if no pre-app sources are configured).
    │
    ▼
Launching     Flutter process has attached (SessionStarted daemon event) and
              app.start has been received (app_id captured). The app is
              building or running for the first time.
              Session::current_progress holds the latest app.progress build
              message (finished:false) for display in the status bar.
    │
    ▼
Running       Set ONLY on the app.started daemon event
              (DaemonMessage::AppStarted). current_progress is cleared.
              When a VM service unavailability hint is present, the handler
              for AppProgress { finished: true } is guarded by
              !vm_service_unavailable so that the hint is not overwritten by
              a late progress-clear event; flush_exception_buffer() runs
              before the guidance entries are appended to the log.
```

The key invariant: process attachment and `app.start` advance the phase to `Launching`; only `app.started` advances it to `Running`. This invariant is enforced on all paths, not just the initial daemon-event path:

- **Auto-reload selection** (`SessionManager::reloadable_sessions()`): only sessions in `Running` are eligible for file-watcher-triggered reloads. A session may have both `app_id` and a live command sender while still in `Launching` (the `app.start` event sets `app_id` before the first build completes), so `app_id` presence alone is not sufficient. The manual `HotReload`/`HotRestart` handlers gate on `is_running()` by the same rule.

- **Reload completion and failure** (`Session::complete_reload()`, `Session::fail_reload()`): both methods act only when the session is in `Reloading`. A `SessionReloadFailed` or `SessionRestartFailed` message arriving while the session is still `Launching` (e.g. because a file change fired an auto-reload during a long first-compile) leaves the phase as `Launching`; it does not promote the session to `Running`.

This matters most on targets with long build windows (Android/Gradle), where the `Launching` phase can span many seconds and a file-watcher event can race with the first `app.started`. On fast targets the app typically reaches `Running` before any file change is detected, which is why the gap was not visible on macOS.

### Install Wizard Step Execution Flow (Phase 2 + Phase 5)

When the user presses Enter on a runnable wizard step (`FlutterSdk` or `PathConfig`):

```
Enter key → Message::InstallWizardRunSelectedStep
    │
    ▼
handler::update() (install_wizard/actions.rs)
    │  calls begin_step(kind): clears install_task, bumps run_seq
    │  mints CancellationToken synchronously
    │  stores InstallTaskHandle { cancel, join: None } on install_task  — Phase 5
    │  resolves FlutterInstallTarget / path_bin_dir from InstallWizardState
    ▼
UpdateAction::RunWizardStep { kind, cancel_token, run_seq, install, path_bin_dir }
    │
    ▼
handle_action() in actions/mod.rs
    │  spawns background task off the Tokio thread pool; reuses cancel_token
    ├── msg_tx.send(Message::WizardInstallTaskReady { kind, run_seq, handle })  — Phase 5
    │     TEA handler validates kind + run_seq; on match, upgrades install_task.join
    ├── msg_tx.send(Message::WizardStepStarted { kind, run_seq })
    │
    ├── [FlutterSdk step]
    │     ensure_disk_space() + check_network_connectivity()           — Phase 5 preflights
    │     install_flutter(target, token, on_event) streams InstallEvent
    │     via run_streaming (git-clone) or download_to_file + extract_archive (archive)
    │     → msg_tx.send(Message::WizardStepLog { kind, line })  — per output line
    │     → msg_tx.send(Message::WizardStepCompleted { kind, sdk_path, .. })
    │       or WizardStepFailed { kind, reason }
    │         (if Error::Cancelled → shows "Cancelled"; otherwise "Failed — retry or r")
    │
    └── [PathConfig step]
          add_to_path(bin_dir, shell) writes shell rc file
          → msg_tx.send(Message::WizardStepCompleted { kind, summary, sdk_path: None })
            or WizardStepFailed { kind, reason }

WizardStepLog → InstallWizardState.push_step_log()   (raw line stored; ANSI stripped at render time by progress.rs)
WizardStepPhase → handle_step_phase() → set_step_phase()  (updates live phase row in StepProgress)
WizardStepCompleted(FlutterSdk, sdk_path=Some(p)):
    1. settings.flutter.sdk_path ← p
    2. installed_sdk_path ← p           (stashed for PathConfig step; cleared after PathConfig succeeds)
    3. UpdateAction::PersistSettings           (write .fdemon/config.toml)
    4. Message::InstallWizardAutoConfigurePath { kind: FlutterSdk }   ← NEW (auto-PATH-config)
       → handle_auto_configure_path():
           begin_step(PathConfig)   (clears install_task, bumps run_seq)
           mints fresh CancellationToken, stores InstallTaskHandle { cancel, join: None }
           dispatches UpdateAction::RunWizardStep { kind: PathConfig, android_sdk_root: None, .. }
           → PathConfig executor: add_to_path(flutter_bin_dir, shell) writes shell rc file
           → WizardStepCompleted(PathConfig) or WizardStepFailed(PathConfig)
               both re-run preflight (Message::InstallWizardRerunPreflight)
               PathConfig completion does NOT re-emit InstallWizardAutoConfigurePath (no loop)
    5. Message::InstallWizardRerunPreflight    (also emitted directly — runs preflight in parallel
       with the auto-PathConfig step, refreshes wizard step statuses)
       → UpdateAction::RunToolchainPreflight   (re-runs checks; Phase 5: also emits SdkResolved)
       → ToolchainPreflightCompleted           (refreshes wizard step statuses)
         Phase 5: handle_preflight_completed checks flutter_now_live();
                  if true and handback_done unset → auto-closes wizard,
                  dispatches DiscoverDevices, sets handback_done
       → UpdateAction::ScanInstalledSdks       (re-scans FVM cache)

WizardStepCompleted(AndroidTools):
    → Message::InstallWizardAutoConfigurePath { kind: AndroidTools }  ← NEW
       → handle_auto_configure_path():
           begin_step(PathConfig)   (clears install_task, bumps run_seq)
           dispatches UpdateAction::RunWizardStep { kind: PathConfig,
               android_sdk_root: Some(resolved_sdk_root), .. }
           → PathConfig executor: add_to_path(flutter_bin_dir, shell)
                                  + add_android_env(shell, platform, sdk_root)
               writes ANDROID_HOME + cmdline-tools/latest/bin, platform-tools, emulator PATH entries
           → WizardStepCompleted/Failed(PathConfig) both re-run preflight
    (seq-guard invariant: begin_step bumps run_seq before any WizardStepStarted arrives,
     so stale WizardStepStarted messages from a prior step are discarded as no-ops)

Esc while a step is Running (Phase 5):
    → Message::InstallWizardCancelStep
      → handle_cancel_step(): fires the synchronously-stored CancellationToken
        on InstallWizardState.install_task and resets execution to Idle.
        If the daemon's cancel confirmation (WizardStepFailed "Cancelled:…") arrives
        before or after Esc, handle_step_failed routes it to StepExecStatus::Cancelled
        (not Failed) — rendering is neutral (no red badge, no "Failed" summary).
        StepExecStatus::Cancelled is a terminal state distinct from Failed; the step
        is still retriable via Enter.

Esc / HideInstallWizard with Flutter live (Phase 5):
    → delegates to close_wizard_and_dispatch_discovery helper
    → sets UiMode::Startup, dispatches DiscoverDevices
```

The TUI `StepProgress` widget (`widgets/install_wizard/progress.rs`) renders the live `StepExecution` state: a progress bar (when `DownloadProgress.total` is known), a byte counter, a phase label (driven by `WizardStepPhase` → `set_step_phase`), and a scrolling ANSI-sanitized tail of the last `MAX_LOG_TAIL` log lines from the bounded `log_tail: VecDeque<String>`. The `RESULT_SUMMARY_HEIGHT` constant controls how many lines are reserved for the step result summary.

### Log Processing Flow

```
FlutterProcess
    │
    ├── stdout reader task ──▶ DaemonEvent::Stdout(line)
    │                              │
    │                              ▼
    │                         protocol::parse_daemon_message()
    │                              │
    │                              ▼
    │                         DaemonEvent::Message(parsed)
    │                              │
    └── stderr reader task ──▶ DaemonEvent::Stderr(line)
                                   │
                                   ▼
                              Message::Daemon(event)
                                   │
                                   ▼
                              handler::update()
                                   │
                                   ▼
                              state.add_log(LogEntry)
                                   │
                                   ▼
                              tui::render() → LogView widget
```

---

## Key Types

### AppState (Model)

The complete application state, owned by the Engine. Contains:
- **UI mode** (`UiMode`) — Normal, DeviceSelector, Loading, etc.
- **Session manager** — Multi-session coordination with up to 9 sessions
- **Device selector state** — Device/emulator selection UI state
- **Configuration** — Settings, project path, project name
- **Active session state** — Phase, logs, log view state, app ID, device info, reload count
- **Mouse region registry** (`mouse_regions: MouseRegionsCell`) — Per-frame click-region table; a TEA-approved render-hint exception (see "Mouse Region Registry" in Key Patterns above). Access via `take_guard()` → `MouseRegionGuard<'a>` (RAII, panic-safe); `take()`/`set()` are low-level primitives for tests only.
- **Mouse capture flag** (`mouse_capture_active: bool`) — Reflects whether terminal mouse capture is currently active. Mutated only by `Message::MouseCaptureChanged` after the TUI runner has performed the actual terminal write.
- **Runner action queue** (`pending_runner_actions: Vec<UpdateAction>`) — Holds `SetMouseCapture` and `WriteClipboard` actions that require synchronous terminal or clipboard I/O. `process.rs` intercepts these two variants and pushes them here instead of forwarding to `handle_action`. The TUI runner drains this queue after each `process_message()` call.

### Message (Events)

All possible events that can affect application state:
- **Input**: `Key(KeyEvent)`, `Daemon(DaemonEvent)`, `Tick`
- **Navigation**: `ScrollUp`, `ScrollDown`, `PageUp`, `PageDown`
- **Control**: `HotReload`, `HotRestart`, `StopApp`
- **Reload lifecycle**: `ReloadStarted`, `ReloadCompleted { time_ms }`, `ReloadFailed { reason }`
- **File watcher**: `FilesChanged { count }`, `AutoReloadTriggered`
- **Session management**: `ShowDeviceSelector`, `DeviceSelected { device }`, `NextSession`, `CloseCurrentSession`
- **Mouse/clipboard**: `MouseCaptureChanged { active }` — sent by the TUI runner after completing a `SetMouseCapture` action; updates `AppState::mouse_capture_active`
- **Toolchain diagnostics**: `ToolchainPreflightCompleted { report: ToolchainReport }` — sent by the background preflight task; the `install_wizard` handler stores the report on `InstallWizardState` and transitions each step to its resolved status
- **Install wizard step execution** (Phase 2): `InstallWizardRunSelectedStep` (Enter key on a runnable step), `WizardStepStarted { kind, run_seq }` (carries the current `run_seq: u64`; `handle_step_started` discards the message as a no-op when `kind` or `run_seq` do not match the current run — the old defensive `begin_step` fallback was removed), `WizardStepLog { kind, line }` (streaming output line from git-clone or precache — raw line stored in bounded `VecDeque`; ANSI codes stripped at render time by the `StepProgress` widget in `progress.rs`), `WizardDownloadProgress { kind, received, total }`, `WizardStepCompleted { kind, summary, sdk_path }`, `WizardStepFailed { kind, reason }`, `WizardStepPhase { kind, label }` (routes to `handle_step_phase` → `set_step_phase`; drives the live phase row in the `StepProgress` widget, replacing the previous dead `[label]` log line approach). `InstallWizardAutoConfigurePath { kind: WizardStepKind }` — emitted by `handle_step_completed` alongside `PersistSettings` after a successful `FlutterSdk` or `AndroidTools` step; routed by `handler/update.rs` to `install_wizard::handle_auto_configure_path`, which begins the PathConfig step automatically (calls `begin_step(PathConfig)`, mints a fresh `CancellationToken`, bumps `run_seq`, dispatches `RunWizardStep { kind: PathConfig, .. }`). PathConfig completion and failure both re-run preflight. PathConfig completion never re-emits `InstallWizardAutoConfigurePath` (no loop). The seq-guard invariant is preserved because `begin_step` bumps `run_seq` before any new `WizardStepStarted` arrives.
- **Lifecycle**: `Quit`

### UpdateResult (Update Output)

The return type from `handler::update()`:
- **message** — Optional follow-up `Message` to process
- **action** — Optional primary `UpdateAction` side effect for the event loop
- **extra_actions** — Additional `UpdateAction` side effects (`Vec<UpdateAction>`). Used when a single handler needs to dispatch more than one action in the same TEA cycle. `handle_open_details` uses this to dispatch both `FetchInspectorProperties` and `FetchLayoutData` together. The engine drains `action` and `extra_actions` through the same hydration and dispatch path.

**UpdateAction variants:**
- `SpawnTask(Task)` — Spawn an async task (reload, restart, etc.)
- `DiscoverDevices` — Trigger device discovery
- `DiscoverEmulators` — Trigger emulator discovery
- `LaunchEmulator { emulator_id }` — Launch a specific emulator
- `SpawnSession { device, config }` — Create a new Flutter session
- `SetMouseCapture(bool)` — Instruct the TUI runner to enable or disable terminal mouse capture. The runner performs the synchronous terminal write outside the TEA pipeline and then sends `Message::MouseCaptureChanged { active }` as a follow-up so the TEA model (`AppState::mouse_capture_active`) reflects the new state. Intercepted by `process.rs` and queued in `AppState::pending_runner_actions` rather than routed through `handle_action`.
- `WriteClipboard { text }` — Instruct the TUI runner to write `text` to the OS clipboard via the runner-owned `Clipboard` implementation. Fire-and-forget from the TEA perspective; a warning toast is shown on failure. Intercepted by `process.rs` and queued in `AppState::pending_runner_actions` rather than routed through `handle_action`.
- `AutoSaveConfig { configs }` — Persist an updated `LoadedConfigs` to `.fdemon/launch.toml` on a background task; used when the New Session Dialog mutates FDemon-owned launch configurations.
- `PersistSettings { settings, project_path }` — Persist the current `Settings` to `.fdemon/config.toml` on a background task. Keeps the TEA event loop unblocked when a settings toggle (e.g. `Shift+H` in the Inspector) flips a persisted boolean. Emits `Message::SettingsPersisted` on success or `Message::SettingsPersistFailed` on failure.
- `FetchLayoutData { session_id, node_id, vm_handle }` — Fetch layout data (constraints, size, flex info) for a widget via `ext.flutter.inspector.getLayoutExplorerNode`. `vm_handle` is hydrated by `process.rs` before dispatch.
- `FetchInspectorProperties { session_id, node_id, vm_handle }` — Fetch widget properties and render-object sub-properties via the two-stage `ext.flutter.inspector.getProperties` pipeline. Dispatched alongside `FetchLayoutData` by `handle_open_details` via `UpdateResult::extra_actions`. `vm_handle` is hydrated by `process.rs` before dispatch.
- `RunToolchainPreflight { project_path, explicit_sdk_path }` — Spawn a background task that calls `fdemon_daemon::toolchain::run_preflight()` and sends `Message::ToolchainPreflightCompleted { report }` when done. Never errors; results are always a `ToolchainReport`. Emitted when `UiMode::InstallWizard` is opened (at startup when no Flutter SDK resolves, or via the `I` keybinding from Normal mode).
- `RunWizardStep { kind, cancel_token, run_seq, install, path_bin_dir }` — Spawn the Phase 2 install executor for a selected wizard step. For `FlutterSdk` steps, `install: Some(FlutterStepParams)` carries the resolved `FlutterInstallTarget` and target directory; for `PathConfig` steps, `path_bin_dir: Some(PathBuf)` carries the Flutter `bin/` directory to add. The executor streams progress back via `WizardStep*` messages. Phase 5: `handle_run_selected_step` mints the `CancellationToken` synchronously and stores it on `InstallWizardState.install_task` (as `InstallTaskHandle { cancel, join: None }`) before dispatching this action, so the token is available for cancellation the instant the step is Running. The action carries the same token clone (`cancel_token`) and the current `run_seq` into the executor; the executor reuses `cancel_token` rather than minting a fresh one, then sends `WizardInstallTaskReady { kind, run_seq, handle }` so the handler can upgrade the stored handle's `join` field after validating the seq. `begin_step` clears any prior `install_task` and bumps `run_seq`; `hide_install_wizard` cancels and clears any in-flight handle. Emitted by `InstallWizardRunSelectedStep` handling when the selected step is `FlutterSdk`, `AndroidTools`, or `PathConfig`.

---

## API Surface

### Public API Boundaries

Each crate in the workspace has a clearly defined public API. Only items exported from `lib.rs` are considered public. Items marked `pub(crate)` are internal implementation details.

#### `fdemon-core` — Domain Types

**Public API** (exported from `lib.rs`):
- `LogEntry`, `LogLevel`, `LogSource` — Log entries and metadata
- `AppPhase` — Application lifecycle phases: `Initializing`, `Preparing` (pre-app sources polling), `Launching` (process attached, building), `Running` (set on `app.started`), `Reloading`, `Stopped`, `Quitting`
- `DaemonMessage`, `DaemonEvent` — Events from Flutter daemon
- `Error`, `Result<T>` — Error handling types
- `is_runnable_flutter_project()`, `discover_flutter_projects()` — Project discovery
- `prelude` module — Common imports
- `DiagnosticsNode`, `LayoutInfo`, `EdgeInsets`, `WidgetSize`, `BoxConstraints` — Widget tree and layout types (`widget_tree.rs`)
- `FlexChild`, `FlexFit`, `Axis`, `MainAxisAlignment`, `CrossAxisAlignment`, `MainAxisSize` — Flex layout types for `Row`/`Column`/`Flex` containers; populated from `getLayoutExplorerNode` responses (`widget_tree.rs`)
- `DetailsContext` — Cached tree-derived visibility predicates for one Details view session; holds `is_flex_layout` (mirrors DevTools' `isFlexLayout`) and `parent_type`; computed by `compute_details_context` and stored on `InspectorState` (`widget_tree.rs`)
- `Location`, `LocationMap`, `RebuildLocation`, `RebuildStatsSnapshot`, `RebuildEventPayload`, `parse_rebuilt_widgets_event` — Widget rebuild telemetry types and parser (`rebuild_stats.rs`)
- `TimelineThread`, `TimelinePhase`, `TimelineEvent`, `parse_vm_timeline` — VM timeline event types and parser (`timeline.rs`)

**Internal** (`pub(crate)`):
- Protocol parsing helpers
- Stack trace implementation details

#### `fdemon-daemon` — Flutter Process Management

**Public API** (exported from `lib.rs`):
- `Device`, `Emulator`, `AndroidAvd`, `IosSimulator` — Device types
- `discover_devices()`, `discover_emulators()`, `launch_emulator()` — Discovery functions
- `FlutterProcess` — Process spawning and lifecycle
- `CommandSender`, `DaemonCommand` — Command dispatch
- `ToolAvailability` — Tool detection

- `run_preflight(project_path, explicit_sdk_path) -> ToolchainReport` — read-only toolchain diagnostics entry point (`toolchain/mod.rs`)
- `ToolchainReport`, `ComponentCheck`, `ComponentStatus`, `ComponentKind`, `HostPlatform`, `HostShell`, `DoctorLine`, `DoctorMarker` — Phase 1 toolchain report types (`toolchain/types.rs`)
- `InstallMethod`, `HostArch`, `FlutterRelease`, `FlutterReleaseManifest`, `FlutterInstallTarget`, `DownloadProgress`, `FlutterInstallOutcome` — Phase 2 install types (`toolchain/types.rs`)
- `install_flutter`, `fetch_release_manifest`, `archive_download_url`, `resolve_install_dir`, `InstallEvent` — Phase 2 Flutter SDK install API (`toolchain/flutter_install.rs`)
- `run_streaming` — Phase 2 child-process line streaming (`toolchain/process_stream.rs`)
- `download_to_file`, `verify_sha256`, `extract_zip`, `extract_tar_xz`, `extract_archive` — Phase 2 download and archive helpers (`toolchain/download.rs`)
- `ensure_disk_space`, `check_network_connectivity` — Phase 5 preflight helpers (`toolchain/download.rs`)
- `PartFileGuard` — Phase 5 RAII partial-file cleanup (`toolchain/download.rs`)
- `add_to_path`, `rc_file_for_shell`, `PathConfigOutcome` — Phase 2 shell rc file PATH writers (`toolchain/path_config.rs`)
- `resolve_android_sdk_root_path(override_path: Option<&Path>) -> PathBuf` — shared Android SDK root resolver; single source of truth for install-time and check-time SDK path resolution (`toolchain/checks/android.rs`, re-exported via `toolchain/mod.rs`)

**Internal** (`pub(crate)`):
- JSON-RPC protocol parsing (`protocol.rs`)
- Request tracking implementation
- AVD/simulator utilities
- Toolchain check implementation details (`toolchain/checks/android.rs` check functions, `toolchain/doctor.rs`)

#### `fdemon-app` — Application State and Orchestration

**Public API** (exported from `lib.rs`):
- `Engine` — Orchestration core
- `EngineEvent` — Domain events for external consumers
- `EnginePlugin` — Extension trait for plugins
- `AppState` — TEA model (read-only access recommended)
- `Message` — TEA messages
- `UpdateAction`, `UpdateResult` — TEA update outputs
- `Session`, `SessionHandle`, `SessionManager` — Session types
- `services::FlutterController` — Reload/restart operations
- `services::LogService` — Log buffer access
- `services::StateService` — App state queries
- `services::Clipboard`, `services::SystemClipboard`, `services::NullClipboard` — Clipboard write trait and runtime implementations (`services::MemoryClipboard` is exported only behind `#[cfg(test)]`)
- `config::Settings`, `config::LaunchConfig` — Configuration types
- `install_wizard::InstallWizardState`, `install_wizard::WizardStep`, `install_wizard::WizardStepKind`, `install_wizard::StepStatus`, `install_wizard::WizardPane` — toolchain diagnostics wizard state types
- `install_wizard::StepExecution`, `install_wizard::StepExecStatus` — Phase 2 per-step execution state (Idle/Running/Succeeded/Failed/Cancelled, log tail, progress bytes). `Cancelled` (Phase 5 followup) is a terminal state distinct from `Failed`: no red badge, no "Failed" styling; the step is still retriable via Enter.
- `install_wizard::InstallTaskHandle` — Phase 5 bundle of `CancellationToken` + `JoinHandle` for the active install task; held on `InstallWizardState.install_task`
- `config::ToolchainSettings` — Phase 2 `[toolchain]` config block (install method, channel, directories)

**Internal** (`pub(crate)`):
- TEA handler implementation (`handler/`)
- Process spawning logic (`process.rs`, `spawn.rs`)
- Version check (`version_check.rs`) — GitHub releases API call; exposed only via `spawn_version_check` in `spawn.rs`
- Signal handling (`signals.rs`)
- Action dispatching (`actions/` — modular directory with `mod.rs`, `session.rs`, `vm_service.rs`, `performance.rs`, `inspector/`, `network.rs`)

#### `fdemon-dap` — DAP Server

**Public API** (exported from `lib.rs`):
- `DapServer`, `DapServerHandle` — TCP server lifecycle
- `DapClientSession`, `NoopBackend` — Session and test backend
- `DapMessage`, `DapRequest`, `DapResponse` — Protocol message types
- `DebugBackend`, `DebugEvent`, `StepMode` (including `Rewind`), `BackendError` — Backend trait and types
- `DapExceptionPauseMode`, `PauseReason` — Pause state enums
- `BreakpointState`, `BreakpointCondition`, `BreakpointResult` — Breakpoint tracking
- `FrameStore`, `VariableStore`, `SourceReferenceStore`, `ScopeKind` — Reference stores and scope kinds
- `ThreadMap`, `MultiSessionThreadMap` — Thread ID mapping
- `ExceptionRef` — Stored exception reference for `exceptionInfo` and exception scope
- `parse_log_message`, `LogSegment` — Logpoint interpolation
- `run_dap_stdio()` — Stdio transport entry point

**Internal** (`pub(crate)`):
- Protocol codec (Content-Length framing)
- Adapter handler methods
- Variable expansion logic (`adapter/variables.rs`)
- Event emission helpers (`adapter/events.rs`)

#### `fdemon-tui` — Terminal UI

**Public API** (exported from `lib.rs`):
- `run_with_project()` — Main TUI entry point
- Widget types are not exported (TUI-specific)

**Internal** (`pub(crate)`):
- All rendering logic
- Terminal setup/cleanup
- Event polling

### Visibility Conventions

| Visibility | Meaning | External Access |
|------------|---------|-----------------|
| `pub` (in `lib.rs`) | Public API | ✅ Stable, documented, supported |
| `pub` (in submodule) | Crate-public | ⚠️ Internal, may change |
| `pub(crate)` | Crate-internal | ❌ Private implementation detail |
| `pub(super)` | Parent module only | ❌ Private implementation detail |
| (no visibility) | Module-private | ❌ Private implementation detail |

**Rule:** External consumers should only use items exported from `lib.rs`. Importing from submodules (e.g., `use fdemon_app::handler::update`) is unsupported and may break.

---

## Extension Points

The Engine provides two extension mechanisms for pro features (MCP server, remote SSH, desktop apps):

### 1. Event Subscription (`Engine::subscribe()`)

Async broadcast channel for observing domain events. Best for read-only consumers that need async processing.

```rust
let mut rx = engine.subscribe();

tokio::spawn(async move {
    while let Ok(event) = rx.recv().await {
        match event {
            EngineEvent::ReloadCompleted { session_id, time_ms } => {
                // Forward to remote client
            }
            EngineEvent::LogBatch { session_id, entries } => {
                // Stream logs
            }
            _ => {}
        }
    }
});
```

**Key Properties:**
- **Non-blocking**: Subscribers receive events via async channel
- **Multiple subscribers**: Each call to `subscribe()` creates a new receiver
- **Lagging policy**: If a subscriber falls behind, older events are dropped
- **Event types**: 15 event types covering sessions, phases, reloads, logs, devices, files

See `engine_event.rs` for the full `EngineEvent` enum.

### 2. Plugin Trait (`EnginePlugin`)

Synchronous lifecycle callbacks for tighter integration. Best for features that need to react to every message or participate in the Engine lifecycle.

```rust
#[derive(Debug)]
struct MetricsPlugin {
    reload_count: AtomicUsize,
}

impl EnginePlugin for MetricsPlugin {
    fn name(&self) -> &str { "metrics" }

    fn on_start(&self, state: &AppState) -> Result<()> {
        // Called when Engine starts
        Ok(())
    }

    fn on_message(&self, msg: &Message, state: &AppState) -> Result<()> {
        // Called after each message is processed
        Ok(())
    }

    fn on_event(&self, event: &EngineEvent) -> Result<()> {
        // Called for each EngineEvent
        if matches!(event, EngineEvent::ReloadCompleted { .. }) {
            self.reload_count.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }

    fn on_shutdown(&self) -> Result<()> {
        // Called during shutdown
        Ok(())
    }
}

// Registration
engine.register_plugin(Box::new(MetricsPlugin { reload_count: AtomicUsize::new(0) }));
engine.notify_plugins_start();
```

**Key Properties:**
- **Synchronous**: Hooks are called inline with message processing
- **Lifecycle**: Covers start, per-message, per-event, shutdown
- **Thread-safe**: Must be `Send + Sync`
- **Error handling**: Plugin errors are logged but don't crash the Engine

### 3. Service Traits

Programmatic access to Flutter operations via trait-based abstractions.

**`FlutterController`** (`services/flutter_controller.rs`):
```rust
if let Some(controller) = engine.flutter_controller() {
    controller.reload().await?;
    controller.restart().await?;
    controller.stop().await?;
    let running = controller.is_running().await;
}
```

**`LogService`** (`services/log_service.rs`):
```rust
let log_service = engine.log_service();
let logs = log_service.get_logs(100).await;
let count = log_service.count().await;
```

**`StateService`** (`services/state_service.rs`):
```rust
let state_service = engine.state_service();
let phase = state_service.phase().await;
let info = state_service.project_info().await;
let running = state_service.is_running().await;
```

**Key Properties:**
- **Trait-based**: Abstracts daemon implementation details
- **Async**: All operations return `async` futures
- **Testable**: Traits can be mocked for testing
- **Thread-safe**: Uses `Arc<SharedState>` internally

### Extension Point Comparison

| Feature | Event Subscription | Plugin Trait | Service Traits |
|---------|-------------------|--------------|----------------|
| **Async** | ✅ Yes | ❌ No | ✅ Yes |
| **Multiple consumers** | ✅ Yes | ✅ Yes | ✅ Yes |
| **Read state** | ✅ Events only | ✅ Full state | ✅ Via services |
| **Write state** | ❌ No | ❌ No | ✅ Commands only |
| **Lifecycle hooks** | ❌ No | ✅ Yes | ❌ No |
| **Best for** | Remote forwarding | Metrics, logging | Control operations |

For detailed examples and usage patterns, see [Extension API Documentation](./EXTENSION_API.md).

---

## Future Considerations

- **Remote MCP Server**: The Engine's event broadcasting and service traits are designed to support an MCP server that can control Flutter Demon from Claude Desktop or other AI tools
- **SSH Remote Development**: The headless mode and shared state architecture enable remote Flutter development workflows
- **Multi-Project Workspaces**: The single-session architecture could be extended to support multiple concurrent projects in a workspace view
- **Time-Travel Debugging**: The TEA pattern (pure update function) enables recording and replaying state transitions for debugging
