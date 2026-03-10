## Task: Add Native Platform Logs to Example App for Manual Testing

**Objective**: Add native Android (Kotlin) and macOS (Swift) logging code to `example/app2` so that running fdemon against the example app produces real native platform log output for manual testing of the native log capture feature.

**Depends on**: None (app changes are independent of fdemon Rust code)

### Scope

- `example/app2/android/app/src/main/kotlin/com/example/flutter_deamon_sample/MainActivity.kt`: Add native Android logging
- `example/app2/android/app/src/main/kotlin/com/example/flutter_deamon_sample/NativeLogDemo.kt`: **NEW** — background logging with multiple tags
- `example/app2/macos/Runner/AppDelegate.swift`: Add native macOS logging
- `example/app2/macos/Runner/NativeLogDemo.swift`: **NEW** — `NSLog` and `os_log` output
- `example/app2/lib/main.dart`: Add a "Native Logs" demo button that triggers platform channel calls

### Details

#### 1. Android — Native Kotlin Logging

Currently `MainActivity.kt` is a bare `FlutterActivity` subclass. Add a method channel that triggers native logging across multiple tags and priorities, simulating real-world plugin behavior.

**`MainActivity.kt`** — add method channel handler:

```kotlin
package com.example.flutter_deamon_sample

import android.util.Log
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    private val CHANNEL = "com.example.flutter_deamon_sample/native_logs"

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)

        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, CHANNEL)
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "triggerNativeLogs" -> {
                        NativeLogDemo.emitSampleLogs()
                        result.success(null)
                    }
                    "startPeriodicLogs" -> {
                        val intervalMs = call.argument<Int>("intervalMs") ?: 2000
                        NativeLogDemo.startPeriodicLogs(intervalMs.toLong())
                        result.success(null)
                    }
                    "stopPeriodicLogs" -> {
                        NativeLogDemo.stopPeriodicLogs()
                        result.success(null)
                    }
                    else -> result.notImplemented()
                }
            }
    }
}
```

**`NativeLogDemo.kt`** — produces logs across multiple tags and priorities:

```kotlin
package com.example.flutter_deamon_sample

import android.util.Log
import java.util.Timer
import java.util.TimerTask

object NativeLogDemo {
    private const val TAG_DEMO = "NativeDemo"
    private const val TAG_PLUGIN = "MyPlugin"
    private const val TAG_GO = "GoLog"
    private const val TAG_NETWORK = "OkHttp"

    private var periodicTimer: Timer? = null
    private var logCounter = 0

    /**
     * Emit a burst of sample native logs across different tags and priorities.
     * These simulate what a real Flutter app with native plugins would produce.
     */
    fun emitSampleLogs() {
        // Simulate a native plugin initialization sequence
        Log.i(TAG_DEMO, "Native log demo triggered from Flutter")
        Log.d(TAG_PLUGIN, "Plugin initializing native components")
        Log.i(TAG_PLUGIN, "Plugin v2.1.0 loaded successfully")

        // Simulate Go/gomobile output (common in cross-platform Flutter apps)
        Log.d(TAG_GO, "Go runtime initialized, GOMAXPROCS=4")
        Log.i(TAG_GO, "gRPC client connected to backend:8443")
        Log.w(TAG_GO, "TLS certificate expires in 7 days")

        // Simulate network library output
        Log.d(TAG_NETWORK, "--> GET https://api.example.com/data")
        Log.d(TAG_NETWORK, "--> END GET")
        Log.d(TAG_NETWORK, "<-- 200 OK (45ms)")
        Log.d(TAG_NETWORK, "<-- END HTTP (1234-byte body)")

        // Simulate various priority levels
        Log.v(TAG_DEMO, "Verbose: detailed trace information")
        Log.d(TAG_DEMO, "Debug: diagnostic information")
        Log.i(TAG_DEMO, "Info: general information")
        Log.w(TAG_DEMO, "Warning: potential issue detected")
        Log.e(TAG_DEMO, "Error: something went wrong (simulated)")

        // Simulate a multi-line native log (stack trace style)
        Log.e(TAG_PLUGIN, "NullPointerException: Attempt to invoke virtual method on null reference")
        Log.e(TAG_PLUGIN, "  at com.example.plugin.DataManager.fetchData(DataManager.kt:42)")
        Log.e(TAG_PLUGIN, "  at com.example.plugin.PluginHandler.handleCall(PluginHandler.kt:87)")
    }

    /**
     * Start emitting periodic native logs (simulates ongoing plugin activity).
     */
    fun startPeriodicLogs(intervalMs: Long) {
        stopPeriodicLogs()
        logCounter = 0
        periodicTimer = Timer().apply {
            scheduleAtFixedRate(object : TimerTask() {
                override fun run() {
                    logCounter++
                    when (logCounter % 4) {
                        0 -> Log.i(TAG_GO, "Heartbeat #$logCounter: connection alive")
                        1 -> Log.d(TAG_NETWORK, "Request #$logCounter: GET /api/status -> 200 (${(10..200).random()}ms)")
                        2 -> Log.i(TAG_PLUGIN, "Event #$logCounter: sensor data received")
                        3 -> Log.d(TAG_DEMO, "Tick #$logCounter: background task running")
                    }
                }
            }, 0L, intervalMs)
        }
    }

    fun stopPeriodicLogs() {
        periodicTimer?.cancel()
        periodicTimer = null
    }
}
```

#### 2. macOS — Native Swift Logging

Currently `AppDelegate.swift` is a bare `FlutterAppDelegate`. Add `NSLog` and `os_log` calls.

**`AppDelegate.swift`** — add method channel handler:

```swift
import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
    override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }

    override func applicationDidFinishLaunching(_ notification: Notification) {
        let controller = mainFlutterWindow?.contentViewController as! FlutterViewController
        let channel = FlutterMethodChannel(
            name: "com.example.flutter_deamon_sample/native_logs",
            binaryMessenger: controller.engine.binaryMessenger
        )

        channel.setMethodCallHandler { (call, result) in
            switch call.method {
            case "triggerNativeLogs":
                NativeLogDemo.emitSampleLogs()
                result(nil)
            case "startPeriodicLogs":
                let args = call.arguments as? [String: Any]
                let intervalMs = args?["intervalMs"] as? Int ?? 2000
                NativeLogDemo.startPeriodicLogs(intervalMs: intervalMs)
                result(nil)
            case "stopPeriodicLogs":
                NativeLogDemo.stopPeriodicLogs()
                result(nil)
            default:
                result(FlutterMethodNotImplemented)
            }
        }

        super.applicationDidFinishLaunching(notification)
    }
}
```

**`NativeLogDemo.swift`** — produces `NSLog` and `os_log` output:

```swift
import Foundation
import os.log

class NativeLogDemo {
    private static let pluginLog = OSLog(subsystem: "com.example.myplugin", category: "general")
    private static let networkLog = OSLog(subsystem: "com.example.network", category: "http")
    private static var timer: Timer?
    private static var counter = 0

    /// Emit a burst of sample native logs using NSLog and os_log.
    /// These simulate what macOS Flutter plugins produce via unified logging.
    static func emitSampleLogs() {
        // NSLog — the classic Objective-C logging (visible in Console.app)
        NSLog("Native log demo triggered from Flutter")
        NSLog("Plugin v2.1.0 loaded — using NSLog for backward compatibility")

        // os_log — modern unified logging with subsystem/category
        os_log("Plugin initializing native components", log: pluginLog, type: .info)
        os_log("Configuration loaded from bundle", log: pluginLog, type: .debug)
        os_log("Sensor framework connected", log: pluginLog, type: .info)
        os_log("Calibration data stale — recalibrating", log: pluginLog, type: .default)

        // Network subsystem logs
        os_log("GET https://api.example.com/data", log: networkLog, type: .debug)
        os_log("Response: 200 OK (45ms)", log: networkLog, type: .debug)
        os_log("TLS certificate expires in 7 days", log: networkLog, type: .default)

        // Error/fault levels
        os_log("Connection timeout after 30s", log: networkLog, type: .error)
        os_log("Critical: data corruption detected (simulated)", log: pluginLog, type: .fault)

        // Multi-line NSLog (simulated stack trace)
        NSLog("Error in native plugin: NullPointerException")
        NSLog("  at MyPlugin.fetchData() (MyPlugin.swift:42)")
        NSLog("  at PluginHandler.handleCall() (PluginHandler.swift:87)")
    }

    /// Start emitting periodic native logs.
    static func startPeriodicLogs(intervalMs: Int) {
        stopPeriodicLogs()
        counter = 0
        let interval = TimeInterval(intervalMs) / 1000.0
        timer = Timer.scheduledTimer(withTimeInterval: interval, repeats: true) { _ in
            counter += 1
            switch counter % 4 {
            case 0:
                os_log("Heartbeat #%d: connection alive", log: pluginLog, type: .info, counter)
            case 1:
                os_log("Request #%d: GET /api/status -> 200", log: networkLog, type: .debug, counter)
            case 2:
                NSLog("Event #%d: sensor data received", counter)
            default:
                os_log("Tick #%d: background task running", log: pluginLog, type: .debug, counter)
            }
        }
    }

    static func stopPeriodicLogs() {
        timer?.invalidate()
        timer = nil
    }
}
```

#### 3. Dart Side — Platform Channel Calls

Add a new demo section to `lib/main.dart` with buttons to trigger native logging:

```dart
// lib/native_logs/native_log_demo.dart
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class NativeLogDemoPage extends StatefulWidget {
  const NativeLogDemoPage({super.key});

  @override
  State<NativeLogDemoPage> createState() => _NativeLogDemoPageState();
}

class _NativeLogDemoPageState extends State<NativeLogDemoPage> {
  static const _channel = MethodChannel(
    'com.example.flutter_deamon_sample/native_logs',
  );
  bool _periodicRunning = false;

  Future<void> _triggerNativeLogs() async {
    try {
      await _channel.invokeMethod('triggerNativeLogs');
    } on PlatformException catch (e) {
      debugPrint('Failed to trigger native logs: ${e.message}');
    }
  }

  Future<void> _togglePeriodicLogs() async {
    try {
      if (_periodicRunning) {
        await _channel.invokeMethod('stopPeriodicLogs');
      } else {
        await _channel.invokeMethod('startPeriodicLogs', {'intervalMs': 2000});
      }
      setState(() => _periodicRunning = !_periodicRunning);
    } on PlatformException catch (e) {
      debugPrint('Failed to toggle periodic logs: ${e.message}');
    }
  }

  @override
  void dispose() {
    if (_periodicRunning) {
      _channel.invokeMethod('stopPeriodicLogs');
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const Padding(
          padding: EdgeInsets.all(16),
          child: Text(
            'Native Platform Logs',
            style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
          ),
        ),
        const Padding(
          padding: EdgeInsets.symmetric(horizontal: 16),
          child: Text(
            'Trigger native logs that are invisible to Flutter\'s --machine mode. '
            'These logs are captured by fdemon\'s native log capture feature.',
          ),
        ),
        const SizedBox(height: 16),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16),
          child: ElevatedButton(
            onPressed: _triggerNativeLogs,
            child: const Text('Emit Native Log Burst'),
          ),
        ),
        const SizedBox(height: 8),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16),
          child: ElevatedButton(
            onPressed: _togglePeriodicLogs,
            style: ElevatedButton.styleFrom(
              backgroundColor: _periodicRunning ? Colors.red : null,
            ),
            child: Text(
              _periodicRunning ? 'Stop Periodic Logs' : 'Start Periodic Logs (2s)',
            ),
          ),
        ),
      ],
    );
  }
}
```

Add this page/section to the existing navigation in `main.dart` alongside the existing demo sections (Counter, Network, Mixed Loggers, etc.).

#### 4. Add `NativeLogDemo.swift` to Xcode project

The new Swift file must be added to the Xcode project's build sources. The simplest approach:
- Open `example/app2/macos/Runner.xcodeproj` in Xcode
- Drag `NativeLogDemo.swift` into the Runner group
- Ensure it's in the "Compile Sources" build phase

Alternatively, if the project uses `Runner.xcodeproj/project.pbxproj`, the file reference and build phase entry can be added manually (but this is error-prone — Xcode is preferred).

### Acceptance Criteria

1. Running `flutter run` on Android and pressing "Emit Native Log Burst" produces `android.util.Log` output visible in `adb logcat` under tags: `NativeDemo`, `MyPlugin`, `GoLog`, `OkHttp`
2. Running `flutter run` on macOS and pressing "Emit Native Log Burst" produces `NSLog` and `os_log` output visible in `log stream`
3. "Start Periodic Logs" produces ongoing native log output every 2 seconds
4. "Stop Periodic Logs" stops the periodic output
5. The native logs are NOT visible in Flutter's `--machine` output (confirming the gap fdemon addresses)
6. The Dart `print()` from `debugPrint` IS visible in `--machine` output (confirming the existing path still works)
7. All log priorities are represented (V/D/I/W/E on Android; debug/info/default/error/fault on macOS)
8. Multiple tags/subsystems are used (simulating real multi-plugin apps)
9. The app builds and runs on both Android and macOS without errors

### Testing

Manual testing only — this task produces test fixtures, not automated tests:

1. **Android verification:**
   ```bash
   cd example/app2
   flutter run -d <android_device>
   # In another terminal:
   adb logcat -v threadtime | grep -E "NativeDemo|MyPlugin|GoLog|OkHttp"
   # Press "Emit Native Log Burst" in the app
   # Verify logs appear in logcat but NOT in fdemon's Flutter log view (before native log feature)
   ```

2. **macOS verification:**
   ```bash
   cd example/app2
   flutter run -d macos
   # In another terminal:
   log stream --predicate 'process == "flutter_deamon_sample"' --level debug
   # Press "Emit Native Log Burst" in the app
   # Verify NSLog/os_log output appears in log stream but NOT in fdemon's Flutter log view
   ```

3. **After native log capture is implemented (tasks 01-08):**
   - Run fdemon against `example/app2`
   - Press "Emit Native Log Burst"
   - Verify native logs appear in fdemon with `[NativeDemo]`, `[GoLog]`, `[OkHttp]` etc. prefixes
   - Verify `[flutter]` tag logs are excluded by default
   - Toggle source filter to `Native` — only native logs visible
   - Start periodic logs — verify ongoing capture works

### Notes

- **This task is independent of the Rust implementation** — it can be done in Wave 1 alongside tasks 01/02/03.
- The method channel name `com.example.flutter_deamon_sample/native_logs` matches the app's package name convention.
- The Android `NativeLogDemo` uses `object` (Kotlin singleton) for simplicity — no lifecycle concerns.
- The macOS `NativeLogDemo` uses `static` methods — same pattern.
- The periodic log feature is useful for testing high-volume native log scenarios and verifying the `LogBatcher` handles the throughput.
- On macOS, `os_log` with subsystem/category produces structured output that the `log stream` parser (task 06) extracts as tags. `NSLog` produces unstructured output that falls back to the `"native"` tag.
- The simulated error logs (NullPointerException stack traces) test multi-line native log rendering.

---

## Completion Summary

**Status:** Not Started
