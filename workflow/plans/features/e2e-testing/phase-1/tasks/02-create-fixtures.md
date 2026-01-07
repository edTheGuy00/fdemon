## Task: Create JSON Fixture Files for Daemon Responses

**Objective**: Create recorded JSON-RPC response fixtures representing real Flutter daemon output for use in mock tests.

**Depends on**: 01-add-dependencies

### Scope

- `tests/fixtures/daemon_responses/` - **NEW** directory with JSON files

### Details

Create JSON fixture files that mirror the exact format Flutter's daemon uses. These will be loaded by `MockFlutterDaemon` to simulate realistic responses.

**Directory Structure:**

```
tests/fixtures/
└── daemon_responses/
    ├── daemon_connected.json
    ├── device_list.json
    ├── app_start_sequence.json
    ├── hot_reload_success.json
    ├── hot_reload_error.json
    └── app_stop.json
```

**File Contents:**

`daemon_connected.json`:
```json
{
  "event": "daemon.connected",
  "params": {
    "version": "0.6.1",
    "pid": 12345
  }
}
```

`device_list.json`:
```json
[
  {
    "id": "emulator-5554",
    "name": "Android SDK built for x86",
    "platform": "android",
    "emulator": true,
    "category": "mobile",
    "platformType": "android",
    "ephemeral": false
  },
  {
    "id": "00008030-001A35E11234802E",
    "name": "iPhone 14 Pro",
    "platform": "ios",
    "emulator": false,
    "category": "mobile",
    "platformType": "ios",
    "ephemeral": false
  }
]
```

`app_start_sequence.json` (array of events in order):
```json
[
  {
    "event": "app.start",
    "params": {
      "appId": "test-app-id",
      "deviceId": "emulator-5554",
      "directory": "/path/to/project",
      "supportsRestart": true
    }
  },
  {
    "event": "app.debugPort",
    "params": {
      "appId": "test-app-id",
      "port": 8181,
      "wsUri": "ws://127.0.0.1:8181/ws"
    }
  },
  {
    "event": "app.started",
    "params": {
      "appId": "test-app-id"
    }
  }
]
```

`hot_reload_success.json`:
```json
[
  {
    "event": "app.progress",
    "params": {
      "appId": "test-app-id",
      "id": "hot.reload",
      "message": "Performing hot reload...",
      "finished": false
    }
  },
  {
    "event": "app.progress",
    "params": {
      "appId": "test-app-id",
      "id": "hot.reload",
      "message": "Reloaded 1 of 1 libraries in 245ms.",
      "finished": true
    }
  }
]
```

`hot_reload_error.json`:
```json
[
  {
    "event": "app.progress",
    "params": {
      "appId": "test-app-id",
      "id": "hot.reload",
      "message": "Performing hot reload...",
      "finished": false
    }
  },
  {
    "event": "app.log",
    "params": {
      "appId": "test-app-id",
      "log": "Compiler message:\nlib/main.dart:10:5: Error: Expected ';' after this.",
      "error": true,
      "stackTrace": null
    }
  }
]
```

`app_stop.json`:
```json
{
  "event": "app.stop",
  "params": {
    "appId": "test-app-id"
  }
}
```

### Acceptance Criteria

1. Directory `tests/fixtures/daemon_responses/` exists
2. All 6 JSON fixture files are present and valid JSON
3. Fixtures match Flutter daemon's actual JSON-RPC format (validated against protocol.rs parsing)
4. Files can be loaded and parsed by `serde_json`

### Testing

```rust
// Verify fixtures are valid JSON that parses to DaemonMessage
#[test]
fn test_fixtures_parse_correctly() {
    let connected = include_str!("../fixtures/daemon_responses/daemon_connected.json");
    let msg = DaemonMessage::parse(connected);
    assert!(matches!(msg, Some(DaemonMessage::DaemonConnected(_))));
}
```

### Notes

- Fixtures are based on Flutter 3.x daemon protocol
- Keep fixtures minimal - only include fields actually used in tests
- Array fixtures (sequences) are for simulating multi-event flows
- These fixtures serve as documentation of the expected protocol

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/fixtures/daemon_responses/daemon_connected.json` | Created JSON fixture for daemon.connected event |
| `tests/fixtures/daemon_responses/device_list.json` | Created JSON fixture with 2 sample devices (Android emulator, iOS physical) |
| `tests/fixtures/daemon_responses/app_start_sequence.json` | Created JSON fixture array with app.start, app.debugPort, app.started sequence |
| `tests/fixtures/daemon_responses/hot_reload_success.json` | Created JSON fixture array for successful hot reload flow |
| `tests/fixtures/daemon_responses/hot_reload_error.json` | Created JSON fixture array for failed hot reload with compile error |
| `tests/fixtures/daemon_responses/app_stop.json` | Created JSON fixture for app.stop event |
| `tests/fixture_parsing_test.rs` | Created integration test file with 7 tests to validate all fixtures |

### Notable Decisions/Tradeoffs

1. **Field Names**: Used camelCase field names (e.g., `appId`, `deviceId`, `supportsRestart`) to match Flutter daemon's actual JSON-RPC protocol, which is validated by the existing `#[serde(rename_all = "camelCase")]` attributes in `src/daemon/events.rs`.

2. **Test Coverage**: Created comprehensive test suite that validates:
   - Each individual fixture parses to the correct `DaemonMessage` variant
   - All fixtures are valid JSON
   - Multi-event sequences (app_start_sequence, hot_reload flows) parse correctly
   - Error conditions (hot_reload_error with error flag) are detected properly

3. **Fixture Structure**: Device list and event sequences are arrays to support testing multi-step flows, while single events (daemon_connected, app_stop) are standalone objects.

### Testing Performed

- `cargo check` - Passed (JSON fixtures integrated without compilation errors)
- `cargo test --lib` - Passed (1249 tests, all unit tests pass)
- `cargo test --test fixture_parsing_test` - Passed (7 new tests, all pass)
- `cargo test --test discovery_integration` - Passed (16 tests, existing integration tests unaffected)
- `cargo fmt --check` - Passed (no formatting issues in new code)

### Risks/Limitations

1. **Pre-existing e2e test issue**: There's a module conflict in `tests/e2e.rs` (file exists at both `tests/e2e.rs` and `tests/e2e/mod.rs`). This was created in task 01 and prevents running `cargo test` on all tests, but does not affect this task's fixtures. The issue should be resolved in a future task.

2. **Pre-existing clippy warning**: There's a clippy warning in `src/app/state.rs:289` about `.is_multiple_of()`. This is unrelated to the fixture creation task and was introduced in a previous commit.

3. **Fixture validation**: All fixtures have been validated against the existing `DaemonMessage::parse()` implementation in `src/daemon/protocol.rs`, ensuring they match the exact format expected by the application.
