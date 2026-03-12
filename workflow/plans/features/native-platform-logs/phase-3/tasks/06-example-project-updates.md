## Task: Example Project READMEs and Custom Source Demo

**Objective**: Update example project READMEs to properly document what each app demonstrates, and add a sample `.fdemon/config.toml` to `example/app2/` showing custom source configuration.

**Depends on**: None (can be done in parallel with implementation tasks)

### Scope

- `example/app1/README.md` — Rewrite from generic Flutter stub to describe the app
- `example/app2/README.md` — Rewrite from generic Flutter stub to describe the app + native logs demo
- `example/app2/.fdemon/config.toml` — **NEW** sample config demonstrating native log settings and custom sources

### Details

#### `example/app1/README.md`

Replace the generic "A new Flutter project" content with a description of what app1 demonstrates:

Content outline:
- **Title**: `Flutter Demon — Example App 1: Dart Logging & Errors`
- **Description**: A sample Flutter app for testing fdemon's log capture, filtering, and error display with Dart-only logging
- **What it demonstrates**:
  - Built-in `print()` / `debugPrint()` / `dart:developer log()`
  - `logger` package integration
  - `talker` package integration
  - Sync and async error capture
  - Stack trace rendering
  - Environment checking
  - Log spam testing (for performance/ring buffer validation)
- **How to run**: `cd example/app1 && fdemon` or `cargo run -- example/app1`
- **No native platform code** — this app is purely Dart-side logging

#### `example/app2/README.md`

Replace the generic content with comprehensive documentation:

Content outline:
- **Title**: `Flutter Demon — Example App 2: Native Platform Logs & Networking`
- **Description**: A sample Flutter app for testing fdemon's native platform log capture, network profiling, and custom log sources
- **What it demonstrates**:
  - **Native platform logs** (Android Kotlin, iOS Swift, macOS Swift):
    - `NativeLogDemoPage` — trigger burst logs, start/stop periodic native logging
    - Android: `android.util.Log` with tags: `NativeDemo`, `MyPlugin`, `GoLog`, `OkHttp` at all priority levels
    - iOS: `NSLog` + `os_log` with subsystems: `com.example.myplugin`, `com.example.network`
    - macOS: Same as iOS with macOS-specific timer scheduling
  - **Network requests**: HTTP GET/POST via `http` package
  - **Mixed logging**: Combined Dart + native log output
  - **Flutter errors**: Widget and async error scenarios
- **How to run**: `cd example/app2 && fdemon` or `cargo run -- example/app2`
- **Testing native logs**:
  - Run on Android: tap "Trigger Native Logs" to emit logcat entries, use `T` key in fdemon to filter tags
  - Run on iOS simulator: same UI, logs appear via `xcrun simctl spawn log stream`
  - Run on macOS: same UI, logs appear via `log stream`
- **Custom source configuration**: Reference the `.fdemon/config.toml` sample

#### `example/app2/.fdemon/config.toml`

Create a sample configuration file demonstrating all native log settings:

```toml
# Flutter Demon Configuration — Example App 2
# This file demonstrates native platform log settings and custom sources.

[native_logs]
# Master toggle for native platform log capture (default: true)
enabled = true

# Tags to exclude from native capture (default: ["flutter"])
# The "flutter" tag is excluded to avoid duplicating logs already captured via --machine
exclude_tags = ["flutter"]

# Minimum log level for native logs (default: "info")
# Options: "verbose", "debug", "info", "warning", "error"
min_level = "info"

# Per-tag level overrides
[native_logs.tags.GoLog]
min_level = "debug"

[native_logs.tags.OkHttp]
min_level = "warning"

# Custom log sources — arbitrary processes whose output is parsed as log entries.
# These run alongside the built-in platform capture (Android logcat, macOS/iOS log stream).

# Example: tail a sidecar log file
# [[native_logs.custom_sources]]
# name = "sidecar"
# command = "tail"
# args = ["-f", "/tmp/my-sidecar.log"]
# format = "raw"

# Example: custom JSON log stream
# [[native_logs.custom_sources]]
# name = "api-server"
# command = "my-log-tool"
# args = ["--follow", "--format", "json"]
# format = "json"

# Example: filtered logcat for a specific tag from another process
# [[native_logs.custom_sources]]
# name = "go-backend"
# command = "adb"
# args = ["logcat", "GoLog:D", "*:S", "-v", "threadtime"]
# format = "logcat-threadtime"
```

### Acceptance Criteria

1. `example/app1/README.md` describes all Dart logging features the app demonstrates
2. `example/app2/README.md` describes native platform log demo, networking, and custom sources
3. Both READMEs include "How to run" instructions
4. `example/app2/.fdemon/config.toml` is valid TOML with commented examples of all native log settings
5. Custom source examples in the config are commented out (so the example works without custom tools installed)
6. READMEs mention the `T` key for tag filtering

### Notes

- Keep READMEs concise and practical — users should be able to scan and understand the app's purpose in 30 seconds
- The `.fdemon/config.toml` custom source examples should be commented out so the app works out of the box. Uncommented settings should only be the ones that enhance the default experience (like per-tag min_level overrides for the demo tags)
- Don't add a `.fdemon/config.toml` to app1 — it's a Dart-only app with no native log configuration needs
