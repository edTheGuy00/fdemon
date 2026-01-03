# Flutter Demon - MCP Server Integration Plan

## TL;DR

Expose Flutter Demon as an **MCP (Model Context Protocol) server** that AI agents (Claude, Cursor, Zed AI, etc.) can connect to for programmatic control of Flutter development workflows. This enables AI assistants to hot reload, restart, view logs, inspect widget trees, and query app state—all through a standardized protocol. We use **Streamable HTTP transport** (localhost binding) so the TUI can run normally while the MCP server accepts connections on a configurable port.

---

## MCP Protocol Overview

The **Model Context Protocol (MCP)** is an open standard introduced by Anthropic in late 2024 that enables LLM applications to integrate with external tools and data sources through a unified interface.

### Protocol Specification Reference

- **Spec Version**: 2025-11-25 (latest)
- **Spec URL**: https://modelcontextprotocol.io/specification/2025-11-25
- **Base Protocol**: JSON-RPC 2.0
- **Transports**: stdio (subprocess) or Streamable HTTP

### Key Concepts

| Concept | Description |
|---------|-------------|
| **Host** | LLM application that initiates connections (e.g., Claude Desktop, Cursor) |
| **Client** | Connector within the host that manages MCP protocol |
| **Server** | Service that provides context and capabilities (Flutter Demon) |
| **Tools** | Model-controlled functions the AI can invoke (reload, restart, etc.) |
| **Resources** | Application-controlled data (logs, state, widget tree) |
| **Prompts** | Templated messages (optional, less relevant for our use case) |

### Why Streamable HTTP (Not stdio)

For Flutter Demon, we **cannot use stdio transport** because:

1. The TUI (Ratatui + crossterm) **owns the terminal's stdin/stdout**
2. MCP stdio transport requires clean stdin/stdout access
3. Both cannot coexist on the same terminal

**Solution**: Use **Streamable HTTP transport**:
- TUI runs normally on the terminal
- MCP server binds to `localhost:<port>` (e.g., `127.0.0.1:3939`)
- AI agents connect via HTTP
- Both can run simultaneously

---

## Proposed MCP Capabilities

### Tools (Model-Controlled Actions)

These are functions the AI model can invoke to control Flutter Demon:

| Tool Name | Description | Input Schema | Output |
|-----------|-------------|--------------|--------|
| `flutter.reload` | Hot reload the running app | `{}` | Success/failure message |
| `flutter.restart` | Hot restart the running app | `{}` | Success/failure message |
| `flutter.stop` | Stop the running Flutter app | `{}` | Confirmation |
| `flutter.start` | Start/run the Flutter app | `{ device?: string }` | Process info |
| `flutter.pub_get` | Run `flutter pub get` | `{}` | Package resolution result |
| `flutter.clean` | Run `flutter clean` | `{}` | Clean result |
| `flutter.open_devtools` | Open DevTools in browser | `{}` | DevTools URL |
| `flutter.get_devices` | List available devices | `{}` | Array of devices |
| `flutter.select_device` | Select a device for running | `{ device_id: string }` | Confirmation |

### Resources (Application-Controlled Data)

These provide read-only context to the AI model:

| Resource URI | Description | MIME Type |
|--------------|-------------|-----------|
| `flutter://logs` | Full log buffer (recent N entries) | `text/plain` |
| `flutter://logs/errors` | Error-level logs only | `text/plain` |
| `flutter://logs/warnings` | Warning-level logs only | `text/plain` |
| `flutter://state` | Current app state (running, building, stopped, error) | `application/json` |
| `flutter://devices` | Available devices with details | `application/json` |
| `flutter://project` | Project info (name, path, SDK version) | `application/json` |
| `flutter://widget-tree` | Widget hierarchy (via debug extension) | `application/json` |
| `flutter://config` | Current configuration | `application/json` |

### Resource Templates

| Template URI | Description |
|--------------|-------------|
| `flutter://logs?filter={pattern}` | Filter logs by regex pattern |
| `flutter://logs?level={level}` | Filter logs by level (error, warning, info) |
| `flutter://logs?since={timestamp}` | Logs since ISO 8601 timestamp |

---

## Architecture

### High-Level Design

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Flutter Demon Process                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────┐     ┌─────────────────┐     ┌─────────────────────────┐   │
│  │   TUI       │     │  Service Layer   │     │    MCP Server           │   │
│  │  (Ratatui)  │────▶│                 │◀────│  (Streamable HTTP)      │   │
│  │             │     │  - FlutterCtrl  │     │                         │   │
│  │ Terminal    │     │  - StateService │     │  localhost:3939         │   │
│  │ stdin/out   │     │  - LogService   │     │                         │   │
│  └─────────────┘     └────────┬────────┘     └─────────────────────────┘   │
│                               │                           │                 │
│                               ▼                           │                 │
│                    ┌─────────────────────┐                │                 │
│                    │   Flutter Daemon     │◀───────────────┘                │
│                    │   (child process)    │                                 │
│                    │   - JSON-RPC         │                                 │
│                    │   - stdout/stderr    │                                 │
│                    └─────────────────────┘                                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    │                               │
             ┌──────▼──────┐                ┌───────▼──────┐
             │   Claude    │                │   Cursor     │
             │   Desktop   │                │   IDE        │
             │             │                │              │
             │  MCP Client │                │  MCP Client  │
             └─────────────┘                └──────────────┘
```

### Service Layer (Shared Between TUI and MCP)

The key architectural insight is introducing a **Service Layer** that both the TUI and MCP server can use:

```rust
// services/flutter_controller.rs
pub trait FlutterController: Send + Sync {
    async fn reload(&self) -> Result<ReloadResult>;
    async fn restart(&self) -> Result<RestartResult>;
    async fn stop(&self) -> Result<()>;
    async fn start(&self, device: Option<&str>) -> Result<StartResult>;
    async fn get_state(&self) -> AppRunState;
}

// services/log_service.rs
pub trait LogService: Send + Sync {
    fn get_logs(&self, filter: Option<LogFilter>) -> Vec<LogEntry>;
    fn subscribe(&self) -> broadcast::Receiver<LogEntry>;
    fn get_errors(&self) -> Vec<LogEntry>;
}

// services/state_service.rs
pub trait StateService: Send + Sync {
    fn get_app_state(&self) -> AppState;
    fn get_devices(&self) -> Vec<Device>;
    fn get_project_info(&self) -> ProjectInfo;
}
```

### Module Structure

```
src/
├── lib.rs
├── main.rs
├── app/
│   ├── mod.rs
│   ├── state.rs
│   └── handler.rs
├── core/
│   └── ...
├── daemon/
│   └── ...
├── tui/
│   └── ...
├── services/                    # NEW: Shared service layer
│   ├── mod.rs
│   ├── flutter_controller.rs   # Commands to Flutter daemon
│   ├── state_service.rs        # Read app state
│   └── log_service.rs          # Access log buffer
└── mcp/                         # NEW: MCP server (future)
    ├── mod.rs
    ├── server.rs               # MCP server setup with rmcp
    ├── tools.rs                # Tool implementations
    ├── resources.rs            # Resource handlers
    └── config.rs               # MCP-specific configuration
```

---

## Crate Dependencies

### Required for MCP Server

```toml
[dependencies]
# MCP SDK (official Rust implementation)
rmcp = { version = "0.12", features = ["server", "transport-sse-server", "macros"] }

# HTTP server for Streamable HTTP transport
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors"] }

# Async utilities (already have tokio)
tokio-stream = "0.1"
```

### Already Present (Reusable)

- `tokio` - Async runtime
- `serde` / `serde_json` - JSON serialization
- `tracing` - Logging (can integrate with MCP logging)

---

## Development Phases

### Phase MCP-1: Service Layer Foundation (Pre-MCP)

**Goal**: Extract shared logic into reusable services before MCP implementation.

**Status**: Should be done during Phase 2-3 of main plan.

**Changes**:
1. Create `services/` module
2. Define `FlutterController` trait and implementation
3. Define `StateService` trait and implementation
4. Define `LogService` trait and implementation
5. Refactor TUI handlers to use services instead of direct access
6. Use `Arc<dyn Service>` for shared access

### Phase MCP-2: HTTP Server Infrastructure

**Goal**: Add HTTP server that runs alongside TUI.

**Duration**: 1 week

**Steps**:
1. Add `axum` and related dependencies
2. Create `mcp/server.rs` with basic HTTP routes
3. Add `--mcp-port` CLI flag (default: 3939)
4. Add `--no-mcp` flag to disable MCP server
5. Health check endpoint (`GET /health`)
6. Graceful shutdown integration

### Phase MCP-3: MCP Protocol Implementation

**Goal**: Implement MCP protocol with rmcp SDK.

**Duration**: 2 weeks

**Steps**:
1. Set up MCP server using rmcp with Streamable HTTP transport
2. Implement `initialize` / capability negotiation
3. Implement `tools/list` and `tools/call`
4. Implement `resources/list` and `resources/read`
5. Add session management (MCP-Session-Id header)
6. Add proper error handling per MCP spec

### Phase MCP-4: Tools Implementation

**Goal**: Implement all Flutter control tools.

**Duration**: 1-2 weeks

**Steps**:
1. `flutter.reload` - Call hot reload via service
2. `flutter.restart` - Call hot restart via service
3. `flutter.stop` / `flutter.start` - Process lifecycle
4. `flutter.pub_get` - Run pub get
5. `flutter.clean` - Run flutter clean
6. `flutter.open_devtools` - Open browser to DevTools URL
7. `flutter.get_devices` / `flutter.select_device` - Device management

### Phase MCP-5: Resources Implementation

**Goal**: Implement all context resources.

**Duration**: 1-2 weeks

**Steps**:
1. `flutter://logs` - Return log buffer contents
2. `flutter://state` - Return current app state as JSON
3. `flutter://devices` - Return device list
4. `flutter://project` - Return project metadata
5. Log filtering via resource templates
6. Subscription support for log updates

### Phase MCP-6: Testing & Documentation

**Goal**: Comprehensive testing and documentation.

**Duration**: 1 week

**Steps**:
1. Unit tests for all tools and resources
2. Integration tests with mock MCP client
3. Test with Claude Desktop MCP configuration
4. Document configuration for popular AI tools
5. Add MCP section to README

---

## Example MCP Tool Implementation (rmcp)

```rust
use rmcp::prelude::*;
use std::sync::Arc;
use crate::services::FlutterController;

#[derive(Clone)]
pub struct FlutterMcpTools {
    controller: Arc<dyn FlutterController>,
}

#[tool]
impl FlutterMcpTools {
    /// Hot reload the running Flutter application
    #[tool]
    async fn flutter_reload(&self) -> Result<String, McpError> {
        match self.controller.reload().await {
            Ok(result) => Ok(format!("Hot reload successful in {}ms", result.duration_ms)),
            Err(e) => Ok(format!("Hot reload failed: {}", e)),
        }
    }

    /// Hot restart the running Flutter application
    #[tool]
    async fn flutter_restart(&self) -> Result<String, McpError> {
        match self.controller.restart().await {
            Ok(result) => Ok(format!("Hot restart successful in {}ms", result.duration_ms)),
            Err(e) => Ok(format!("Hot restart failed: {}", e)),
        }
    }

    /// Get the current state of the Flutter application
    #[tool]
    async fn flutter_get_state(&self) -> Result<serde_json::Value, McpError> {
        let state = self.controller.get_state().await;
        Ok(serde_json::to_value(state).unwrap())
    }
}
```

---

## Configuration

### CLI Flags

```
flutter-demon [OPTIONS] [PROJECT_PATH]

MCP Server Options:
    --mcp-port <PORT>    Port for MCP server [default: 3939]
    --no-mcp             Disable MCP server
    --mcp-only           Run MCP server only (no TUI, headless mode)
```

### Configuration File

```toml
# flutter-demon.toml

[mcp]
enabled = true
port = 3939
# Bind address (localhost only for security)
bind = "127.0.0.1"
# Maximum concurrent sessions
max_sessions = 5
# Session timeout in seconds
session_timeout = 3600
```

### Claude Desktop Configuration

To connect Claude Desktop to Flutter Demon:

```json
{
  "mcpServers": {
    "flutter-demon": {
      "url": "http://localhost:3939/mcp",
      "transport": "streamable-http"
    }
  }
}
```

---

## Security Considerations

### Localhost-Only Binding

- MCP server **MUST** bind to `127.0.0.1` only, never `0.0.0.0`
- Prevents remote access to Flutter Demon controls

### Origin Validation

Per MCP spec, the server **MUST** validate the `Origin` header:
```rust
async fn validate_origin(origin: Option<&str>) -> Result<(), McpError> {
    match origin {
        None => Ok(()), // Local requests may not have Origin
        Some(o) if o.starts_with("http://localhost") => Ok(()),
        Some(o) if o.starts_with("http://127.0.0.1") => Ok(()),
        Some(o) if o.starts_with("vscode-webview://") => Ok(()), // VS Code
        Some(_) => Err(McpError::forbidden("Invalid origin")),
    }
}
```

### Rate Limiting

Implement rate limiting for destructive operations:
- Max 10 reloads per second
- Max 5 restarts per minute
- Log all tool invocations for audit

### User Consent

The TUI **SHOULD** display indicators when MCP clients are connected and when tools are invoked, following MCP's human-in-the-loop principle.

---

## Architectural Groundwork for MVP

These patterns should be adopted **now** during MVP development to make MCP integration easier later:

### 1. Service Layer Pattern (Phase 2+)

Instead of TUI handlers directly manipulating daemon:
```rust
// ❌ Don't do this
fn handle_reload(daemon: &mut Daemon) {
    daemon.send_command("reload");
}

// ✅ Do this
fn handle_reload(controller: &dyn FlutterController) {
    controller.reload();
}
```

### 2. Shared State with Arc<RwLock> (Phase 2+)

```rust
pub struct SharedState {
    pub app_state: Arc<RwLock<AppState>>,
    pub log_buffer: Arc<RwLock<LogBuffer>>,
    pub devices: Arc<RwLock<Vec<Device>>>,
}
```

### 3. Command/Query Separation

```rust
// Commands (mutations)
pub enum FlutterCommand {
    Reload,
    Restart,
    Stop,
    Start { device: Option<String> },
    PubGet,
    Clean,
}

// Queries (reads)
pub enum FlutterQuery {
    GetState,
    GetDevices,
    GetLogs(LogFilter),
    GetProjectInfo,
}
```

### 4. Event Broadcasting

Ensure the event system can have multiple subscribers:
```rust
// Allow MCP to subscribe to events
let (event_tx, _) = broadcast::channel::<AppEvent>(100);
// TUI subscribes
let tui_rx = event_tx.subscribe();
// Future: MCP subscribes
let mcp_rx = event_tx.subscribe();
```

---

## Edge Cases & Risks

### Concurrent Access

- **Risk**: TUI and MCP both try to reload simultaneously
- **Mitigation**: Use command queue with deduplication, one reload at a time

### Session Management

- **Risk**: Orphaned MCP sessions consume resources
- **Mitigation**: Session timeout, max session limits, cleanup on TUI exit

### State Synchronization

- **Risk**: MCP sees stale state
- **Mitigation**: Real-time state via broadcast channels, not polling

### Error Propagation

- **Risk**: Daemon errors not properly surfaced via MCP
- **Mitigation**: Map internal errors to MCP error codes consistently

### Graceful Shutdown

- **Risk**: MCP sessions left hanging on TUI exit
- **Mitigation**: Terminate all sessions and close HTTP server before process exit

---

## Success Criteria

### Phase MCP-1 Complete When:
- [ ] Service layer abstraction exists
- [ ] TUI uses services, not direct daemon access
- [ ] Services are thread-safe (`Send + Sync`)

### Phase MCP-3 Complete When:
- [ ] MCP server starts on configured port
- [ ] `initialize` handshake works
- [ ] Tools list returns all tools
- [ ] Resources list returns all resources

### Phase MCP-4 Complete When:
- [ ] `flutter.reload` works via MCP
- [ ] `flutter.restart` works via MCP
- [ ] Claude Desktop can control Flutter Demon

### MCP Feature Complete When:
- [ ] All tools implemented and tested
- [ ] All resources implemented and tested
- [ ] Works with Claude Desktop, Cursor, and Zed
- [ ] Documentation complete
- [ ] Security audit passed (localhost only, origin validation)

---

## Further Considerations

1. **Widget Inspector Integration**: Can we expose the full widget inspector tree via MCP resources? Would require Flutter debug extensions.

2. **Remote MCP (Future)**: If users want remote access, consider adding authentication (JWT, API keys) and TLS. Out of scope for initial implementation.

3. **Bidirectional Events**: MCP 2025-11-25 supports server-to-client notifications. Could push log events, state changes to connected clients.

4. **Multiple Flutter Sessions**: When we add multi-session support (Phase 5), MCP tools will need session specifiers.

5. **Conflict Resolution**: What happens when MCP and TUI user issue conflicting commands? Need clear priority/queuing strategy.

---

## References

- [MCP Specification 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25)
- [rmcp Rust SDK](https://docs.rs/rmcp)
- [MCP Tools Specification](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)
- [MCP Resources Specification](https://modelcontextprotocol.io/specification/2025-11-25/server/resources)
- [MCP Transports](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)