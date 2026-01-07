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
