## Task: Update docs/ARCHITECTURE.md with Custom Source Subsystem

**Objective**: Update the Native Log Capture Subsystem section in `docs/ARCHITECTURE.md` to document the custom source runner, format parser dispatch, and how custom sources integrate with the existing capture pipeline.

**Depends on**: 04-app-custom-source-integration (implementation should be complete so docs match reality)

### Scope

- `docs/ARCHITECTURE.md` — Update the "Native Log Capture Subsystem" section

### Details

The existing section (around lines 1004-1062) documents:
- The `NativeLogCapture` trait
- Platform backends (Android, macOS, iOS)
- Tag filtering and per-tag config
- Tool dependency table

Add the following to this section:

#### Custom Source Runner

Document the new `CustomLogCapture` struct and how it fits into the existing architecture:

```markdown
### Custom Log Sources

Users can define arbitrary log source processes via `[[native_logs.custom_sources]]` configuration. Each custom source implements the same `NativeLogCapture` trait as platform backends.

#### Architecture

```
┌─────────────────────────────────────────────────┐
│              NativeLogCapture trait              │
├─────────────────────────────────────────────────┤
│ AndroidLogCapture │ MacOsLogCapture │ IosLogCapture │ CustomLogCapture │
│ (adb logcat)      │ (log stream)    │ (idevice/     │ (user-defined    │
│                   │                 │  simctl)       │  command)        │
└─────────────────────────────────────────────────┘
                          │
                    NativeLogEvent
                          │
              ┌───────────┴───────────┐
              │  Format Parser        │
              │  (formats.rs)         │
              │  Raw│Json│Logcat│Syslog│
              └───────────────────────┘
```

#### Format Parser Dispatch (`native_logs/formats.rs`)

The `formats` module provides pluggable output parsing for custom sources:

| Format | Parser | Description |
|--------|--------|-------------|
| `raw` | `parse_raw()` | Each line → message (Info level, tag = source name) |
| `json` | `parse_json()` | JSON objects with flexible field names |
| `logcat-threadtime` | Delegates to `android::parse_threadtime_line()` | Android logcat threadtime format |
| `syslog` | Delegates to `macos::parse_syslog_line()` | macOS/iOS unified logging compact format |

Custom sources integrate with the existing pipeline:
- Events flow through the same `NativeLogEvent` → `Message::NativeLog` → handler path
- Tags are tracked in `NativeTagState` and appear in the tag filter overlay
- `should_include_tag()` filtering applies identically
- `min_level` filtering uses the same `effective_min_level()` logic
```

#### Module Reference Update

Add `custom.rs` and `formats.rs` to the module listing:

```markdown
| `native_logs/custom.rs` | `CustomLogCapture` — spawns user-defined commands, reads stdout through format parsers |
| `native_logs/formats.rs` | `parse_line()` dispatch — Raw, JSON, Logcat, Syslog format parsers |
```

#### Session Handle Update

Document the `custom_source_handles` field:

```markdown
`SessionHandle` stores:
- `native_log_shutdown_tx` — platform capture shutdown signal
- `native_log_task_handle` — platform capture async task
- `native_tag_state` — discovered tags and visibility state
- `custom_source_handles: Vec<CustomSourceHandle>` — one per configured custom source
```

### Acceptance Criteria

1. Custom source architecture documented in the Native Log Capture Subsystem section
2. Format parser dispatch table added
3. Architecture diagram updated to show `CustomLogCapture` alongside platform backends
4. Module reference includes `custom.rs` and `formats.rs`
5. `SessionHandle` fields section updated with `custom_source_handles`
6. Content is consistent with the actual implementation

### Testing

- No automated tests (documentation only)
- Verify architecture diagram and module references match the implemented code

### Notes

- Keep the existing content intact — only add new subsections and update existing tables/diagrams
- The architecture section is technical reference for developers, not user-facing — use implementation details and type names
- Follow the existing documentation style: ASCII diagrams, tables for structured data, code snippets for key types

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Added "Custom Log Sources" subsection to Native Log Capture Subsystem (architecture diagram, format parser dispatch table, lifecycle messages table, design decisions); added `custom.rs` and `formats.rs` to module reference table; expanded `SessionHandle` fields diagram in Multi-Session Architecture |

### Notable Decisions/Tradeoffs

1. **Architecture diagram shows full pipeline**: The ASCII diagram extends past `NativeLogEvent` to show `Message::NativeLog` → `handler::update()` → `NativeTagState + log buffer`, making the integration with the existing pipeline explicit and self-contained within the subsection.

2. **SessionHandle diagram expanded (not a new section)**: The task asked to document `custom_source_handles` under `SessionHandle`. Rather than creating a separate `SessionHandle` fields section in the Native Log Capture Subsystem, the existing `SessionHandle` tree diagram in the Multi-Session Architecture section was expanded to include all task/shutdown handle pairs (vm, perf, network, debug, native_log, custom_source). This is consistent with the file's style and avoids duplication.

3. **Lifecycle messages table added**: `CustomSourceStarted` and `CustomSourceStopped` are documented in a table with their exact field names, matching the actual `message.rs` definitions. This was not explicitly listed in the task spec's section template but was called out in the acceptance criteria ("How custom source events integrate with the existing pipeline").

4. **`syslog` platform caveat documented**: The format table notes that `OutputFormat::Syslog` returns `None` on non-macOS platforms (matching the `#[cfg(not(target_os = "macos"))]` stub in `formats.rs`).

### Testing Performed

- Visual inspection of all three edit locations confirmed correct placement and style alignment
- `grep` verification confirmed all key terms present: `CustomLogCapture`, `custom_source_handles`, `formats.rs`, `custom.rs`, `CustomSourceStarted`, `CustomSourceStopped`, `parse_line`, `OutputFormat`
- File structure verified: 1475 lines total, heading hierarchy intact

### Risks/Limitations

1. **Documentation-only change**: No automated tests; correctness relies on cross-referencing the actual implementation files (`custom.rs`, `formats.rs`, `handle.rs`, `message.rs`).
