# Flutter Demon — Example App 2: Native Platform Logs & Networking

A sample Flutter app for testing fdemon's native platform log capture,
network profiling, and custom log source configuration. Unlike app1, this
app ships native code on Android (Kotlin), iOS (Swift), and macOS (Swift)
that emits logs outside of Flutter's `--machine` stream — exactly the gap
that fdemon's native log capture feature addresses.

## What it demonstrates

### Native platform logs

The **Native Platform Logs** card at the bottom of the main screen connects
to platform-specific logging APIs via a method channel:

| Platform | APIs used | Tags / subsystems |
|----------|-----------|-------------------|
| Android  | `android.util.Log` | `NativeDemo`, `MyPlugin`, `GoLog`, `OkHttp` |
| iOS      | `NSLog` + `os_log` | `com.example.myplugin`, `com.example.network` |
| macOS    | `NSLog` + `os_log` | `com.example.myplugin`, `com.example.network` |

Two actions are available:

- **Emit Native Log Burst** — fires a one-shot sequence covering every
  priority level (verbose/debug/info/warning/error/fault) and simulates
  common plugin patterns: plugin initialisation, gRPC heartbeats, OkHttp
  request/response lines, and a multi-line native stack trace.
- **Start/Stop Periodic Logs (2s)** — emits one log every 2 seconds in a
  rotating pattern across all four tags, simulating ongoing plugin activity.

### Network requests

The **Network Requests** card makes real HTTP calls using the `http` package
to public test APIs (JSONPlaceholder, httpbin.org, Dog CEO, Cat Facts):

- All standard methods: GET, POST, PUT, PATCH, DELETE
- Custom headers, delayed responses, 404 and 500 error responses
- Concurrent burst (6 simultaneous requests)
- "Run All" sequential demo

### Mixed Dart logging

- `logger` + `talker` side by side, all log levels
- `simulateRequestFlow` — shows how Dart logs and network logs interleave
- Verbose spam (20 / 50 messages) for ring buffer testing
- Timer-based delayed logs (5 messages, 1 second apart)

### Flutter errors

- `FlutterError.reportError` (framework error path)
- `PlatformException` from a method channel call
- Layout overflow (`RenderFlex`) triggered live
- Build-time exception with in-tree error recovery

## How to run

```bash
cd example/app2
fdemon
# or from the repo root:
cargo run -- example/app2
```

## Testing native log capture

### Android (logcat)

1. Connect a device or start an emulator.
2. Run: `fdemon` — fdemon starts `adb logcat` automatically.
3. Tap **Emit Native Log Burst** — entries with tags `NativeDemo`, `MyPlugin`,
   `GoLog`, and `OkHttp` appear in the fdemon log view alongside Dart logs.
4. Press `T` in fdemon to open the tag filter overlay and toggle individual
   tags on/off.

### iOS simulator

1. Start an iOS simulator.
2. Run: `fdemon` — fdemon spawns `xcrun simctl spawn booted log stream`.
3. Tap **Emit Native Log Burst** — `NSLog` and `os_log` entries appear with
   their subsystem names as the tag.
4. Press `T` to filter by subsystem.

### macOS

1. Run: `fdemon` (macOS is the host, no separate device step).
2. fdemon runs `log stream --process <pid>` scoped to the Flutter process.
3. Tap **Emit Native Log Burst** — same `os_log` entries as iOS.
4. Press `T` to filter by subsystem.

## Custom source configuration

See `.fdemon/config.toml` in this directory for a sample configuration that:

- Sets a global minimum log level for native logs
- Applies per-tag level overrides (e.g. show `GoLog` debug messages but only
  `OkHttp` warnings)
- Shows commented-out examples of custom log sources (tail a file, stream
  JSON, or run a second `adb logcat` with different tag filters)
