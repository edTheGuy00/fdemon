## Task: Update Documentation for `shared` Config Option

**Objective**: Update `docs/CONFIGURATION.md` and `docs/ARCHITECTURE.md` to document the `shared` field on custom sources and the shared source data flow.

**Depends on**: 01-config-shared-field

### Scope

- `docs/CONFIGURATION.md`: Add `shared` field to custom source reference
- `docs/ARCHITECTURE.md`: Add shared source data flow section

### Details

#### 1. CONFIGURATION.md Updates

Add `shared` to the custom source properties table:

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `shared` | bool | false | Spawn once, shared across all sessions. Logs broadcast to all active sessions. |

Add example config:

```toml
# Shared backend server — spawned once, all sessions see its logs
[[native_logs.custom_sources]]
name = "backend"
command = "python3"
args = ["server/server.py"]
format = "raw"
start_before_app = true
shared = true
ready_check = { type = "http", url = "http://127.0.0.1:8085/health" }
```

Add a "Shared vs Per-Session Sources" subsection explaining:
- Per-session (default): each Flutter session gets its own process
- Shared: one process for the entire project, logs visible in all sessions
- Use `shared = true` for backend servers, databases, and services that bind to a specific port
- Shared sources persist until fdemon quits (not tied to session lifecycle)

#### 2. ARCHITECTURE.md Updates

In the "Custom Log Sources" or "Native Log Capture" section, add:

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

Contrast with per-session sources:

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

### Acceptance Criteria

1. `shared` field documented in CONFIGURATION.md custom source properties table
2. Example config with `shared = true` added
3. "Shared vs Per-Session" explanation added
4. ARCHITECTURE.md updated with shared source data flow diagram
5. No broken markdown links

### Notes

- Keep the documentation concise — one paragraph explaining the distinction, one example config, one flow diagram
- The CONFIGURATION.md section for custom sources was already updated in Phase 1 — extend it, don't rewrite
