# Plan: Native Platform Logs

## TL;DR

Surface native platform logs alongside Flutter logs in fdemon. Flutter's `--machine` mode only forwards Dart `print()` output — native logs from plugins, Go packages, and the engine itself are invisible on most platforms. The gap varies significantly by platform: Android has the biggest hole (entire logcat stream filtered away), macOS has a meaningful gap (NSLog/os_log from plugins invisible), while Linux, Windows, and Web are already well-covered through existing stdout/stderr pipes. We'll run parallel native log capture processes where needed, parse and tag the output, and integrate it into the existing log pipeline with source-based filtering.

---

## Background

### The Problem

Flutter's `--machine` JSON-RPC protocol (`app.log` events) only carries Dart-level `print()`/`debugPrint()` output. The severity of this limitation varies by platform:

- **Android (critical gap):** The Flutter tools' `AdbLogReader` explicitly filters logcat for lines matching the `"flutter"` tag and discards everything else. `android.util.Log.*` calls from Kotlin/Java plugins, Go/gomobile output (`GoLog` tag), system framework logs — all invisible.
- **iOS (moderate gap):** `NSLog()` and `os_log()` from Swift/ObjC plugins — partially captured via lldb attachment but not structured as separate sources.
- **macOS (moderate gap):** `NSLog`/`os_log` from native macOS plugins do NOT flow through the stdout/stderr pipe. Flutter engine `FML_LOG` messages don't appear in Console.app either (open issue #159743). The `DesktopLogReader` only captures stdout/stderr.
- **Linux (minimal gap):** stdout/stderr from both Dart and native GTK plugins (`g_message()`, `g_warning()`) flow through the same pipe. Only deliberate `journald` writes (rare) are missed.
- **Windows (minimal gap):** stdout/stderr captured correctly when piped via `flutter run --machine`. `OutputDebugString` calls (rare in Flutter plugins) are not captured. No gap in practice.
- **Web (no gap for fdemon):** `console.log()` from Dart `print()` is forwarded via DWDS as `app.log` events. `dart:developer.log()` compiles to `console.debug()` which may be filtered (DWDS-level issue, not fixable by fdemon). Log truncation on web is a known framework bug.

### Current State

fdemon has 6 `LogSource` variants: `App`, `Daemon`, `Flutter`, `FlutterError`, `Watcher`, `VmService`. Filtering cycles through `All → App → Daemon → Flutter → Watcher → All` via `LogSourceFilter`. There is no concept of dynamic/custom log sources, and no native process log capture.

### Platform Gap Summary

| Platform | What `--machine` captures | What's missing | Severity |
|----------|--------------------------|----------------|----------|
| Android | Dart `print()` only (logcat filtered to `flutter` tag) | All non-`flutter` logcat tags (GoLog, Kotlin plugins, system) | **Critical** |
| iOS | Dart `print()` + partial native via lldb | Some `os_log`/`NSLog` from plugins not structured | **Moderate** |
| macOS | Dart `print()` + stderr | `NSLog`/`os_log` from native plugins, `FML_LOG` engine msgs | **Moderate** |
| Linux | Dart `print()` + stderr + GTK plugin stderr | Only deliberate `journald` writes (very rare) | **Minimal** |
| Windows | Dart `print()` + stderr (when piped) | `OutputDebugString` (rare in Flutter plugins) | **Minimal** |
| Web | `console.log()` via DWDS | `console.debug()` may be filtered; truncation issues | **None** (DWDS-level) |

---

## Affected Modules

### Core Types (`fdemon-core`)
- `crates/fdemon-core/src/types.rs` — Extend `LogSource` enum, add `NativeTag` type, redesign `LogSourceFilter`
- `crates/fdemon-core/src/types.rs` — **NEW** native log parsing types (or new file `native_log.rs`)

### Daemon Layer (`fdemon-daemon`)
- `crates/fdemon-daemon/src/native_logs/` — **NEW** module: native log process management
  - `mod.rs` — Public API, `NativeLogProcess` trait
  - `android.rs` — `adb logcat` spawning, threadtime format parsing
  - `macos.rs` — `log stream` spawning, unified logging parsing
  - `ios.rs` — `idevicesyslog` / `devicectl` spawning, output parsing
- `crates/fdemon-daemon/src/tool_availability.rs` — Add `adb`, `log` (macOS), `idevicesyslog`/`devicectl` checks

### App Layer (`fdemon-app`)
- `crates/fdemon-app/src/session/handle.rs` — Add `native_log_shutdown_tx` + `native_log_task_handle`
- `crates/fdemon-app/src/actions/native_logs.rs` — **NEW** action: spawn/stop native log capture
- `crates/fdemon-app/src/handler/daemon.rs` — Route `NativeLogLine` events into session log buffer
- `crates/fdemon-app/src/handler/update.rs` — Handle `Message::NativeLog` variant
- `crates/fdemon-app/src/state.rs` — Track native log capture state per session
- `crates/fdemon-app/src/config/settings.rs` — Native log settings (enabled, default filter, custom tags)

### TUI Layer (`fdemon-tui`)
- `crates/fdemon-tui/src/widgets/log_view/` — Render native log source tags with colors
- `crates/fdemon-tui/src/theme/palette.rs` — Add native source colors

---

## Key Technical Findings

### Android (`adb logcat`)

- **Format:** `threadtime` — `MM-DD HH:MM:SS.mmm PID TID PRIO TAG : message`
- **PID filtering:** `adb logcat --pid=$(adb shell pidof -s <package>)` — captures ALL tags from the app process
- **Tag filtering:** `adb logcat flutter:I GoLog:D *:S` — whitelist specific tags
- **Priority levels:** V(erbose), D(ebug), I(nfo), W(arning), E(rror), F(atal)
- **Device targeting:** `adb -s <serial> logcat` — the `session.device_id` is directly usable as the serial
- **Gotcha:** PID changes on app restart; need to re-resolve or fall back to tag-based filtering

### iOS

- **Pre-iOS 17:** `idevicesyslog --udid <udid> --process "Runner"` — filters by process name
- **iOS 17+:** `xcrun devicectl device stream logs --device <udid>` — newer API, syntax in flux
- **Simulators:** `xcrun simctl spawn <udid> log stream --level debug --predicate 'process == "Runner"'`
- **Gotcha:** `idevicesyslog` broken with Xcode 26+; Flutter itself is migrating to `devicectl`/`lldb`-based streaming
- **Gotcha:** iOS via `flutter run` already captures native logs via lldb attachment — some overlap with `--machine` output possible

### macOS (`log stream`)

- **Mechanism:** `log stream --predicate 'process == "YourApp"' --level debug` captures unified logging output
- **What it catches:** `NSLog()` from ObjC plugins, `os_log()` from Swift plugins, Flutter engine `FML_LOG` messages
- **What `--machine` already captures:** Dart `print()` + stderr (covers Dart layer fully)
- **The gap:** Native plugin `NSLog`/`os_log` calls do NOT flow through stdout/stderr pipe — they go to the unified logging system only
- **Known Flutter issues:** `FML_LOG` messages don't appear in Console.app (issue #159743, open P3); `flutter logs -d macos` broken since Flutter 3.16+ (issue #138974, open P2)
- **Process identification:** Filter by process name (app binary name) — available from `session.device_name` or derivable from the Flutter project name

### Linux

- **No meaningful gap.** The `DesktopLogReader` captures stdout + stderr. Native GTK plugins that use `g_message()`/`g_warning()` write to stderr, which is captured. Only deliberate `sd_journal_send()` calls (extremely rare for Flutter plugins) would be missed.
- **Recommendation:** No native log capture needed. Document as "already covered."

### Windows

- **No meaningful gap when run via `flutter run --machine`.** stdout/stderr are piped correctly. `OutputDebugString` calls from native C++ plugins are not captured, but this is rare in the Flutter ecosystem.
- **Standalone release builds** lose all console output (Windows GUI subsystem), but fdemon always uses `flutter run` so this doesn't apply.
- **Recommendation:** No native log capture needed. Document as "already covered."

### Web (Chrome)

- **No gap for fdemon.** `console.log()` from Dart `print()` is forwarded via DWDS as `app.log` events. `dart:developer.log()` compiles to `console.debug()` which may be filtered — this is a DWDS/framework bug, not something fdemon can address.
- **Recommendation:** No native log capture needed. Could potentially connect to Chrome DevTools Protocol `Runtime.consoleAPICalled` for richer capture in the future, but this is low priority.

### Go/gomobile

- Logs appear under the `GoLog` tag on Android logcat
- On iOS/macOS, Go stdout is captured by lldb attachment / stdout pipe (already partially visible)
- `GoLog` tag is reliable when `golang.org/x/mobile/app` is imported; broken without it in some gomobile bind configurations

### What `--machine` Mode Misses (Complete Matrix)

| Source | Android | iOS | macOS | Linux | Windows | Web |
|--------|---------|-----|-------|-------|---------|-----|
| Dart `print()` | Via `app.log` | Via `app.log` | Via `app.log` | Via `app.log` | Via `app.log` | Via DWDS |
| `dart:developer log()` | Via VM Service | Via VM Service | Via VM Service | Via VM Service | Via VM Service | Partial (DWDS) |
| Native plugin logs | **MISSING** | Partial (lldb) | **MISSING** | Captured (stderr) | Captured (stderr) | N/A |
| Go/gomobile (`GoLog`) | **MISSING** | Partial (lldb) | Captured (stdout) | Captured (stdout) | Captured (stdout) | N/A |
| Flutter engine native | **MISSING** | Partial (lldb) | **MISSING** | Captured (stderr) | Captured (stderr) | N/A |
| System framework | **MISSING** | **MISSING** | **MISSING** | N/A | N/A | N/A |

---

## Design Decisions

### Decision 1: Dynamic vs Fixed Log Sources

**Chosen: Hybrid approach** — Add a `LogSource::Native { tag: String }` variant that carries the platform tag name dynamically.

Rationale: Native log tags are arbitrary strings chosen by plugin authors. A fixed enum cannot anticipate all possible tags (`GoLog`, `MyPlugin`, `OkHttp`, etc.). The tag string becomes the filterable/displayable identity.

### Decision 2: PID-based vs Tag-based Filtering (Android)

**Chosen: PID-based as default, tag-based as fallback/override.**

- PID-based (`--pid`) captures everything from the app process without needing to know tag names upfront
- Tag-based allows users to add specific tags from other processes (e.g., system services) via config
- If `pidof` fails (app not yet running), fall back to unfiltered with tag whitelist

### Decision 3: When to Start Native Log Capture

**Chosen: After `AppStarted` event** (not at session creation).

Rationale: The app PID is only knowable after the app is running. Starting `adb logcat` before the app launches would either require tag-only filtering or capture irrelevant pre-launch noise. The `AppStarted` daemon event already triggers VM Service connection — native log capture fits the same lifecycle.

### Decision 4: Deduplication with Flutter Logs

Native logcat includes Flutter's own `flutter:` tag output, which overlaps with `app.log` events from `--machine` mode. We need to either:
- **Filter out the `flutter` tag** from native log capture (simple, loses some native Flutter engine logs)
- **Deduplicate** similar to existing VM Service dedup (complex, timing-sensitive)

**Chosen: Filter out `flutter` tag by default**, with a config option to include it. The `flutter` tag output is already well-covered by the `--machine` protocol. Users who want raw Flutter engine logs can opt in.

### Decision 5: Platform Prioritization

**Chosen: Android (Phase 1) → macOS (Phase 1b) → iOS (Phase 2). Skip Linux/Windows/Web.**

- **Android first:** Biggest gap, most straightforward solution (`adb logcat`), most requested
- **macOS alongside or shortly after Android:** Shares Apple's unified logging infrastructure with iOS (`log stream`), moderate gap with native plugins, and fdemon developers are likely on macOS
- **iOS in Phase 2:** Complex (3+ different tools depending on iOS/Xcode version), Flutter's lldb attachment already captures much of the native output
- **Linux/Windows/Web: No action needed.** stdout/stderr pipe coverage is already sufficient. Document as "covered by existing capture"

### Decision 6: macOS Process Identification

**Chosen: Filter by process name derived from the Flutter project.**

The macOS `log stream` command accepts `--predicate 'process == "YourApp"'`. The process name is the binary name, which corresponds to the Flutter app's product name (typically the project name or a configured name). This is derivable from the project directory or from the `AppStarted` event metadata.

---

## Development Phases

### Phase 1: Android + macOS Native Logs (Core Infrastructure)

**Goal**: Capture and display native log output alongside Flutter logs on the two platforms with meaningful gaps, with source-based filtering.

#### Steps

1. **Extend Core Types**
   - Add `LogSource::Native { tag: String }` variant to `LogSource` enum
   - Add `NativeLogPriority` enum (V/D/I/W/E/F) with mapping to `LogLevel`
   - Redesign `LogSourceFilter` to support `Native` source (cycle: `All → App → Daemon → Flutter → Native → Watcher → All`)
   - Update `FilterState::matches()` to handle `Native` variant
   - Update `LogSource::prefix()` and `Display` impl for `Native` — display as `[tag]` (e.g., `[GoLog]`, `[OkHttp]`, `[MyPlugin]`)

2. **Android Logcat Process (`fdemon-daemon`)**
   - Create `crates/fdemon-daemon/src/native_logs/` module
   - `android.rs`: Spawn `adb -s <serial> logcat --pid=<pid> -v threadtime` as async process
   - Parse threadtime format: extract timestamp, PID, TID, priority, tag, message
   - Regex: `^(\d{2}-\d{2}) (\d{2}:\d{2}:\d{2}\.\d{3})\s+(\d+)\s+(\d+)\s+([VDIWEF])\s+([^:]+?)\s*:\s*(.*)$`
   - Emit `NativeLogEvent { tag, priority, message, timestamp }` through a channel
   - Handle process lifecycle: restart on crash, stop on session end
   - Add `adb` to `ToolAvailability` checks

3. **macOS Unified Logging Process (`fdemon-daemon`)**
   - `macos.rs`: Spawn `log stream --predicate 'process == "<app_name>"' --level debug --style syslog`
   - Parse syslog-style output: extract timestamp, process, subsystem/category (if present), level, message
   - Map macOS log levels (Default/Info/Debug/Error/Fault) to `LogLevel`
   - Use subsystem or category as the "tag" when available (e.g., `com.example.myplugin`), fall back to `"native"` for untagged messages
   - Gate behind `#[cfg(target_os = "macos")]` — fdemon is only built on macOS for macOS desktop targets
   - Emit through same `NativeLogEvent` channel type as Android

4. **Shared Native Log Infrastructure (`fdemon-daemon`)**
   - `mod.rs`: Define `NativeLogEvent { tag: String, level: LogLevel, message: String, timestamp: Option<String> }`
   - Define `NativeLogCapture` trait with `spawn()` → channel + shutdown handle
   - Platform dispatch: based on `session.platform`, instantiate the right capture backend

5. **App Layer Integration**
   - Add `Message::NativeLog { session_id, event }` variant
   - Add `native_log_shutdown_tx` / `native_log_task_handle` to `SessionHandle`
   - Create `actions/native_logs.rs`: spawn native log capture after `AppStarted`
     - Android: resolve PID via `adb shell pidof`, spawn logcat
     - macOS: resolve process name, spawn `log stream`
     - Linux/Windows/Web: no-op (already covered by existing pipe)
   - Route `NativeLogEvent` → `LogEntry { source: Native { tag }, level, message }` → `session.add_log()`
   - Filter out `flutter` tag by default on Android (configurable)

6. **TUI Rendering**
   - Add `palette::SOURCE_NATIVE` color
   - Update `source_style()` to handle `Native` variant
   - Display tag as `[GoLog]`, `[MyPlugin]`, `[com.example.plugin]`, etc. in log view

7. **Configuration**
   - Add to `.fdemon/config.toml`:
     ```toml
     [native_logs]
     enabled = true                    # master toggle
     exclude_tags = ["flutter"]        # tags to exclude (avoid duplication with --machine)
     # include_tags = ["GoLog"]        # optional: only show these tags (overrides exclude)
     min_level = "info"                # minimum priority level
     ```

**Milestone**: Android users can see native logcat output (Go, Kotlin, Java plugin logs). macOS users can see native plugin `NSLog`/`os_log` output. Both filterable by the `Native` source filter. Linux/Windows/Web continue to work as before with no degradation.

---

### Phase 2: iOS Native Logs + Per-Tag Filtering UI

**Goal**: Add iOS native log capture and per-tag filtering for granular control over native log sources.

#### Steps

1. **iOS Log Capture (`fdemon-daemon`)**
   - `ios.rs`: Detect available tools at runtime via `ToolAvailability`
   - Simulator: `xcrun simctl spawn <udid> log stream --level debug --predicate 'process == "Runner"'`
   - Physical device (pre-Xcode 26): `idevicesyslog --udid <udid> --process "Runner"`
   - Physical device (Xcode 26+): `xcrun devicectl device stream logs --device <udid>` (follow Flutter's PR #173724 approach)
   - Parse output format (varies by tool), emit `NativeLogEvent`
   - Shared infrastructure with macOS (`log stream` parsing reusable for simulators)
   - Add `idevicesyslog` / `devicectl` to `ToolAvailability`

2. **Per-Tag Filtering UI**
   - Track discovered tags per session (as native logs arrive, build a `HashSet<String>` of seen tags)
   - Add a tag filter popup/overlay (similar to existing filter cycling but with dynamic tag list)
   - Allow toggling individual tags on/off
   - Keyboard shortcut for tag filter (e.g., `T` for tag filter overlay)

3. **Enhanced Configuration**
   - Add per-tag priority thresholds:
     ```toml
     [native_logs.tags.GoLog]
     min_level = "debug"

     [native_logs.tags.OkHttp]
     min_level = "warning"
     ```

**Milestone**: Full native log support on Android, macOS, and iOS. Users can filter by individual native tags, addressing the original user request for Go tag filtering.

---

### Phase 3: Custom Log Sources (Future)

**Goal**: Allow users to define arbitrary log source processes for any platform.

#### Steps

1. **Custom source configuration:**
   ```toml
   [[native_logs.custom_sources]]
   name = "go-backend"
   command = "adb logcat GoLog:D *:S -v threadtime"
   format = "logcat-threadtime"   # or "raw", "json", "syslog"
   ```

2. **Generic process runner** that spawns user-defined commands and parses output
3. **Named source tags** that appear in the filter UI as first-class sources
4. This is the escape hatch for platforms where we don't have built-in support, or for non-standard logging setups (custom log aggregators, remote log streams, etc.)

**Milestone**: Users can define completely custom log sources for any native process or tool.

---

### Platforms Not Requiring Native Log Capture

These platforms are already well-served by the existing `--machine` stdout/stderr pipe:

- **Linux**: Native GTK plugin output goes to stderr, which is captured. Only deliberate `sd_journal_send()` calls (very rare) are missed.
- **Windows**: stdout/stderr piped correctly via `flutter run --machine`. `OutputDebugString` not captured but extremely rare in Flutter plugins.
- **Web**: `console.log()` forwarded via DWDS. Remaining gaps (`console.debug()` filtering, log truncation) are framework-level issues in DWDS/Flutter, not addressable by fdemon.

---

## Edge Cases & Risks

### Process Lifecycle (Android)
- **Risk:** App restarts change the PID, making `--pid` filter stale
- **Mitigation:** Monitor for `AppStarted` events on hot restart; re-resolve PID and restart logcat process. Also detect logcat process exit and attempt reconnection.

### Process Identification (macOS)
- **Risk:** macOS `log stream` filters by process name, but the process name may not match expectations (e.g., if the app has a custom `CFBundleName` or if multiple instances are running)
- **Mitigation:** Use the process name from the `AppStarted` event or derive from the Flutter project's `Runner.app` binary. For ambiguity, also filter by PID if obtainable.

### Tool Availability
- **Risk:** `adb` not on PATH, `idevicesyslog` not installed, `log` command not available
- **Mitigation:** Use existing `ToolAvailability` infrastructure. Gracefully degrade — show a one-time info message that native logs are unavailable. Never block session startup.

### Log Volume
- **Risk:** Unfiltered logcat can produce thousands of lines/sec, overwhelming the ring buffer. macOS `log stream` can also be noisy.
- **Mitigation:** PID-based filtering on Android (dramatically reduces volume). Process-name filtering on macOS. Additionally, apply priority floor (default: Info+, configurable). The existing 10,000-entry ring buffer and `LogBatcher` (16ms/100-entry) already handle high throughput.

### iOS Tool Fragmentation
- **Risk:** 3+ different iOS log tools with varying compatibility across iOS/Xcode versions
- **Mitigation:** Phase 2 — design for it but ship Android + macOS first. Use runtime detection of available tools. Follow Flutter's own migration path (they're solving the same problem in PR #173724).

### macOS Unified Logging Noise
- **Risk:** macOS `log stream` captures all unified logging from the process, including system framework messages that may not be relevant
- **Mitigation:** Default to `--level info` (skip debug/default). Users can lower to debug in config. The `exclude_tags` config can filter out noisy subsystems.

### Tag Explosion
- **Risk:** Android logcat has hundreds of system tags; even PID-filtered output may have many tags. macOS subsystems can also be numerous.
- **Mitigation:** Exclude known-noisy tags/subsystems by default. The per-tag filter UI (Phase 2) lets users manage this. Start with a sensible default exclude list.

### Deduplication with Existing Streams
- **Risk:** Flutter `print()` output appears both in `app.log` (via `--machine`) and in logcat (as `flutter:I` tag). On macOS, some stdout messages may also appear in unified logging.
- **Mitigation:** Exclude `flutter` tag from native capture by default. Configurable override for users who want raw engine logs.

### Multiple Devices / Serial Targeting
- **Risk:** Multiple connected Android devices — `adb logcat` without `-s` targets the wrong device
- **Mitigation:** Always pass `-s <device_id>` which is already available on `Session`. This is equivalent to what `flutter run -d <device>` does.

### Cross-Platform Build Considerations
- **Risk:** macOS `log stream` is only available on macOS. Adding it may complicate cross-compilation.
- **Mitigation:** Gate macOS native log code behind `#[cfg(target_os = "macos")]`. The existing `ToolAvailability` already uses this pattern for `xcrun simctl`. fdemon's macOS desktop log capture only makes sense when fdemon itself runs on macOS.

---

## Configuration Additions

```toml
# .fdemon/config.toml

[native_logs]
enabled = true                          # Master toggle (default: true)
exclude_tags = ["flutter"]              # Exclude these tags (default: ["flutter"])
# include_tags = []                     # If set, ONLY show these tags (overrides exclude)
min_level = "info"                      # Minimum priority level (default: "info")
                                        # Options: "verbose", "debug", "info", "warning", "error"

# Per-tag overrides (Phase 2)
# [native_logs.tags.GoLog]
# min_level = "debug"
```

---

## Keyboard Shortcuts Summary

| Key | Action | Phase |
|-----|--------|-------|
| Existing source filter cycle | Now includes `Native` in rotation | Phase 1 |
| `T` (in log view) | Open tag filter overlay | Phase 2 |

---

## Success Criteria

### Phase 1 Complete When:
- [ ] Android native logcat output appears in fdemon log view with `[tag]` prefix
- [ ] macOS native `NSLog`/`os_log` output appears in fdemon log view
- [ ] GoLog, Kotlin plugin, and Java plugin logs are visible alongside Flutter logs
- [ ] `LogSourceFilter` can toggle native logs on/off
- [ ] `flutter` tag excluded by default to avoid duplication
- [ ] PID-based filtering works on Android; falls back gracefully if PID unavailable
- [ ] Process-name filtering works on macOS
- [ ] Native log capture starts after `AppStarted` and stops on session end
- [ ] Tool availability checked at startup; graceful degradation if tools missing
- [ ] Linux/Windows/Web sessions are unaffected (no native capture attempted)
- [ ] Configurable via `[native_logs]` section in config.toml
- [ ] All new code has unit tests
- [ ] No regressions in existing log pipeline

### Phase 2 Complete When:
- [ ] iOS native logs captured on physical devices and simulators
- [ ] Per-tag filter UI allows toggling individual tags
- [ ] Per-tag priority thresholds configurable
- [ ] Works across iOS 15+ / Xcode 15+ (graceful degradation for older versions)

---

## Future Enhancements

- **Web CDP integration**: Connect to Chrome DevTools Protocol `Runtime.consoleAPICalled` for richer web log capture (captures `console.debug`, `console.warn`, etc. with full type info)
- **Windows `OutputDebugString` capture**: If demand arises, could use `DebugView`-style capture via the Windows Debug API. Currently no demand.
- **Linux `journald` capture**: If demand arises, could use `journalctl --user -f` with process filtering. Currently no demand since GTK stderr is already captured.

---

## References

### Android
- [Android logcat documentation](https://developer.android.com/tools/logcat)
- [flutter/flutter #120711 — Configurable logcat filter](https://github.com/flutter/flutter/issues/120711)
- [flutter/engine PR #3335 — setLogTag API](https://github.com/flutter/engine/pull/3335)

### iOS / macOS
- [flutter/flutter #159743 — FML_LOG not in Console.app on macOS](https://github.com/flutter/flutter/issues/159743)
- [flutter/flutter #138974 — `flutter logs -d macos` broken](https://github.com/flutter/flutter/issues/138974)
- [flutter/flutter PR #173724 — devicectl/lldb log streaming](https://github.com/flutter/flutter/pull/173724)
- [libimobiledevice idevicesyslog](https://manpages.debian.org/experimental/libimobiledevice-utils/idevicesyslog.1.en.html)
- [simple_native_logger package](https://pub.dev/packages/simple_native_logger) — community workaround for macOS/iOS native log gap

### Desktop / Web
- [flutter/flutter #31521 — Desktop stdout/stderr capture](https://github.com/flutter/flutter/issues/31521)
- [flutter/flutter PR #31874 — Add stderr to desktop log reader](https://github.com/flutter/flutter/pull/31874)
- [flutter/flutter #77002 — Windows console attachment](https://github.com/flutter/flutter/issues/77002)
- [flutter/flutter #47913 — dart:developer.log() on web](https://github.com/flutter/flutter/issues/47913)
- [Chrome DevTools Protocol - Log domain](https://chromedevtools.github.io/devtools-protocol/tot/Log/)

### Cross-Platform
- [flutter/flutter #2303 — dart:io stdout not redirected](https://github.com/flutter/flutter/issues/2303)
- [flutter/flutter #147141 — Logging inconsistencies across platforms](https://github.com/flutter/flutter/issues/147141)
- [golang/mobile — GoLog tag](https://github.com/golang/mobile)
- [package:dwds](https://pub.dev/packages/dwds) — Dart Web Debug Service
