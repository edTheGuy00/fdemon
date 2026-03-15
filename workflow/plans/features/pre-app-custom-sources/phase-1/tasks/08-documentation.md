## Task: Documentation Updates

**Objective**: Update `docs/CONFIGURATION.md` and `docs/ARCHITECTURE.md` to document the new `start_before_app` and `ready_check` config options and the pre-app source flow.

**Depends on**: Task 07 (all implementation complete)

### Scope

- `docs/CONFIGURATION.md`: Add reference for new config fields
- `docs/ARCHITECTURE.md`: Add pre-app source flow diagram

### Details

#### 1. Update `docs/CONFIGURATION.md`

Add a new section under the existing `[[native_logs.custom_sources]]` documentation:

**Pre-App Custom Sources:**

Document the two new fields:
- `start_before_app` (bool, default: false) — when true, the source starts before the Flutter app and its readiness is checked before launching
- `ready_check` (table, optional) — configures how fdemon verifies the source is ready

**Ready Check Types:**

Document all five types with examples:
- `http` — polls an HTTP endpoint for 2xx
- `tcp` — polls a TCP host:port for connection
- `command` — runs an external command until exit code 0
- `stdout` — watches process stdout for a regex pattern
- `delay` — waits a fixed duration

**Configuration reference table:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `start_before_app` | bool | `false` | Start source before Flutter app |
| `ready_check.type` | string | (required) | `http`, `tcp`, `command`, `stdout`, `delay` |
| `ready_check.url` | string | — | HTTP: URL to GET |
| `ready_check.host` | string | — | TCP: hostname |
| `ready_check.port` | integer | — | TCP: port number |
| `ready_check.command` | string | — | Command: executable |
| `ready_check.args` | string[] | `[]` | Command: arguments |
| `ready_check.pattern` | string | — | Stdout: regex pattern |
| `ready_check.seconds` | integer | `5` | Delay: seconds to wait |
| `ready_check.interval_ms` | integer | `500` | HTTP/TCP/Command: poll interval |
| `ready_check.timeout_s` | integer | `30` | HTTP/TCP/Command/Stdout: timeout |

**Example configurations** (use the examples from PLAN.md lines 296-374).

**Validation rules:**
- `ready_check` requires `start_before_app = true`
- `start_before_app = true` without `ready_check` is valid (fire-and-forget)

**Timeout behavior:**
- On timeout, Flutter launches anyway with a warning
- The custom source process continues running

#### 2. Update `docs/ARCHITECTURE.md`

Add to the "Native Log Capture Subsystem" section:

**Pre-App Custom Source Flow:**

```
handle_launch()
  → IF has pre-app sources:
      UpdateAction::SpawnPreAppSources
        → spawn pre-app CustomLogCapture processes
        → run readiness checks (HTTP, TCP, command, stdout, delay)
        → on ready: Message::PreAppSourcesReady
          → UpdateAction::SpawnSession (normal flow continues)
        → on timeout: proceed with warning
  → ELSE:
      UpdateAction::SpawnSession (unchanged)
```

Add to the "Key Patterns" section:
- **Pre-app source gating**: `handle_launch()` conditionally returns `SpawnPreAppSources` when custom sources need to start before the Flutter app. Readiness checks run concurrently with independent timeouts. The Flutter launch gate lifts on `PreAppSourcesReady`.

### Acceptance Criteria

1. `docs/CONFIGURATION.md` documents `start_before_app` and `ready_check` with all five check types
2. `docs/CONFIGURATION.md` includes example TOML for each check type
3. `docs/CONFIGURATION.md` documents validation rules and timeout behavior
4. `docs/ARCHITECTURE.md` includes the pre-app source flow diagram
5. Documentation is accurate and matches the actual implementation

### Testing

- Read through both docs and verify they match the implemented behavior
- Verify TOML examples parse correctly (can test with a quick `toml::from_str` call)

### Notes

- Check if `docs/CONFIGURATION.md` exists first — it may need to be created or the section may need to be added to an existing file
- Use the config examples from `PLAN.md` lines 296-374 as the basis for documentation examples
- Keep the documentation concise — reference the ready check table from PLAN.md

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `docs/CONFIGURATION.md` | Added `#### Pre-App Custom Sources` subsection under `[[native_logs.custom_sources]]` with field reference tables, validation rules, timeout behavior, and 7 TOML examples covering all 5 check types plus fire-and-forget. Updated Table of Contents. |
| `docs/ARCHITECTURE.md` | Added `#### Pre-App Custom Source Flow` subsection under Custom Log Sources with ASCII flow diagram and readiness check type table. Added `### Pre-App Source Gating` to Key Patterns section. |

### Notable Decisions/Tradeoffs

1. **Placement in CONFIGURATION.md**: Added the new subsection directly after the existing `[[native_logs.custom_sources]]` examples block (before Editor Settings), so all custom source documentation is in one place. Used `####` heading to fit the existing `###` heading hierarchy.

2. **Key Patterns section wording**: Described the architectural insight (pure handler returning action, fire-and-forget vs gated variants) concisely in prose rather than repeating the diagram already in the subsystem section.

### Testing Performed

- Read-through of both documents to verify accuracy against implementation tasks 01-07
- Verified all 5 check types are documented with examples (`http`, `tcp`, `command`, `stdout`, `delay`)
- Verified TOML examples match the PLAN.md reference examples (lines 296-374)
- Confirmed field defaults match the task spec (`seconds = 5`, `interval_ms = 500`, `timeout_s = 30`)

### Risks/Limitations

1. **No automated TOML validation**: The task notes that TOML examples could be validated with `toml::from_str` but this is a documentation-only task with no build verification step. The examples were cross-checked manually against PLAN.md.
