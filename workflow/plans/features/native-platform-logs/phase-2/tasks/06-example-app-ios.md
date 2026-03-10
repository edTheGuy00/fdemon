## Task: Add iOS Native Logging to Example App

**Objective**: Create the iOS platform directory in `example/app2` and add Swift native logging code (`NativeLogDemo.swift`) so that running fdemon against the example app on an iOS simulator or device produces real native platform log output for manual testing.

**Depends on**: None (app changes are independent of fdemon Rust code)

### Scope

- `example/app2/`: Run `flutter create --platforms=ios .` to generate the iOS platform directory
- `example/app2/ios/Runner/AppDelegate.swift`: Add method channel handler for native log demo
- `example/app2/ios/Runner/NativeLogDemo.swift`: **NEW** — `NSLog` and `os_log` output, matching macOS implementation
- `example/app2/ios/Runner.xcodeproj/project.pbxproj`: Add `NativeLogDemo.swift` to build sources

### Details

#### 1. Generate iOS platform directory

The `ios/` directory does not exist in `example/app2`. Generate it:

```bash
cd example/app2
flutter create --platforms=ios .
```

This creates the standard Flutter iOS project structure:
- `ios/Runner/` — app source files
- `ios/Runner/AppDelegate.swift` — entry point
- `ios/Runner/Info.plist` — app configuration
- `ios/Runner.xcodeproj/` — Xcode project
- `ios/Runner.xcworkspace/` — Xcode workspace (with CocoaPods)
- `ios/Podfile` — CocoaPods dependencies

#### 2. Modify `AppDelegate.swift`

Replace the generated `AppDelegate.swift` with a version that registers the native log method channel. Follow the same pattern as the macOS `AppDelegate.swift`:

```swift
import Flutter
import UIKit

@main
@objc class AppDelegate: FlutterAppDelegate {
    override func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        let controller = window?.rootViewController as! FlutterViewController
        let channel = FlutterMethodChannel(
            name: "com.example.flutter_deamon_sample/native_logs",
            binaryMessenger: controller.binaryMessenger
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

        GeneratedPluginRegistrant.register(with: self)
        return super.application(application, didFinishLaunchingWithOptions: launchOptions)
    }
}
```

**Key difference from macOS**: On iOS, the `FlutterViewController` is accessed via `window?.rootViewController` (not `mainFlutterWindow?.contentViewController`). The `binaryMessenger` is accessed directly on the controller (not via `.engine.binaryMessenger`).

#### 3. Create `NativeLogDemo.swift`

Create `example/app2/ios/Runner/NativeLogDemo.swift`. This is nearly identical to the macOS version — both platforms share the `Foundation` and `os.log` frameworks:

```swift
import Foundation
import os.log

class NativeLogDemo {
    private static let pluginLog = OSLog(subsystem: "com.example.myplugin", category: "general")
    private static let networkLog = OSLog(subsystem: "com.example.network", category: "http")
    private static var timer: Timer?
    private static var counter = 0

    /// Emit a burst of sample native logs using NSLog and os_log.
    /// These simulate what iOS Flutter plugins produce via unified logging.
    static func emitSampleLogs() {
        // NSLog — the classic logging mechanism (visible in idevicesyslog and Console.app)
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

        // Timer must be created on the main thread for RunLoop scheduling
        DispatchQueue.main.async {
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
    }

    static func stopPeriodicLogs() {
        timer?.invalidate()
        timer = nil
    }
}
```

**iOS-specific note**: The `Timer.scheduledTimer` must run on the main thread's `RunLoop` on iOS (wrapped in `DispatchQueue.main.async`), unlike macOS where the AppKit main thread's RunLoop is always available. This ensures the timer fires correctly.

#### 4. Add `NativeLogDemo.swift` to Xcode project

The new Swift file must be added to `ios/Runner.xcodeproj/project.pbxproj`:
1. Add a `PBXFileReference` entry for `NativeLogDemo.swift`
2. Add the file reference to the `Runner` group's children
3. Add a `PBXBuildFile` entry
4. Add the build file to the Runner target's `PBXSourcesBuildPhase`

Use unique UUIDs that don't collide with existing entries. Follow the same pattern used for the macOS `NativeLogDemo.swift` in the phase 1 task (UUID prefix `FDAE1B0`). For iOS, use a different prefix (e.g., `FDAE2B0`):

```
// PBXBuildFile section
FDAE2B012044A3C60003C045 /* NativeLogDemo.swift in Sources */ = {isa = PBXBuildFile; fileRef = FDAE2B002044A3C60003C045 /* NativeLogDemo.swift */; };

// PBXFileReference section
FDAE2B002044A3C60003C045 /* NativeLogDemo.swift */ = {isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = NativeLogDemo.swift; sourceTree = "<group>"; };

// PBXGroup — Runner group children (add alongside AppDelegate.swift, etc.)
FDAE2B002044A3C60003C045 /* NativeLogDemo.swift */,

// PBXSourcesBuildPhase — files array
FDAE2B012044A3C60003C045 /* NativeLogDemo.swift in Sources */,
```

#### 5. Dart side — no changes needed

The Dart `NativeLogDemoPage` widget in `lib/native_logs/native_log_demo.dart` already works on all platforms via the same method channel (`com.example.flutter_deamon_sample/native_logs`). No Dart code changes are needed.

### Acceptance Criteria

1. `ios/` directory exists in `example/app2` with standard Flutter iOS project structure
2. Running `flutter build ios --simulator` succeeds (or `flutter run -d <ios-simulator>`)
3. Pressing "Emit Native Log Burst" in the app produces `NSLog` and `os_log` output
4. Native logs visible in `xcrun simctl spawn booted log stream --predicate 'process == "Runner"'` (simulator)
5. Native logs visible in `idevicesyslog -p Runner` (physical device, if available)
6. Both `os_log` subsystems (`com.example.myplugin`, `com.example.network`) produce output
7. "Start Periodic Logs" produces ongoing native log output every 2 seconds
8. "Stop Periodic Logs" stops the periodic output
9. All log levels are represented (debug/info/default/error/fault via os_log; Notice via NSLog)
10. The app builds and runs on iOS simulator without errors
11. No changes to Dart code required — existing `native_log_demo.dart` works as-is

### Testing

Manual testing only — this task produces test fixtures, not automated tests:

1. **iOS simulator verification:**
   ```bash
   cd example/app2
   flutter run -d <ios-simulator>
   # In another terminal:
   xcrun simctl spawn booted log stream --predicate 'process == "Runner"' --style syslog --level debug
   # Press "Emit Native Log Burst" in the app
   # Verify NSLog/os_log output appears in log stream
   ```

2. **iOS physical device verification (if device available):**
   ```bash
   cd example/app2
   flutter run -d <physical-device>
   # In another terminal:
   idevicesyslog -u <udid> -p Runner
   # Press "Emit Native Log Burst" in the app
   # Verify native logs appear
   ```

3. **After native log capture is implemented (tasks 01-05):**
   - Run fdemon against `example/app2` with an iOS simulator
   - Press "Emit Native Log Burst"
   - Verify native logs appear in fdemon with `[com.example.myplugin]`, `[com.example.network]` prefixes
   - Verify `[Flutter]` tag logs are excluded by default
   - Start periodic logs — verify ongoing capture works

### Notes

- **`flutter create --platforms=ios .`** is the cleanest way to generate the iOS directory. It respects the existing `pubspec.yaml` and doesn't modify other platform directories. Must be run from within the `example/app2` directory.
- **CocoaPods**: The generated `ios/Podfile` may need `pod install` run afterward. The implementor should run `cd ios && pod install` if the build fails with missing pods.
- **iOS deployment target**: The default Flutter iOS deployment target should be sufficient. No need to change it.
- **`GeneratedPluginRegistrant.register(with: self)`**: This call must come after the method channel is set up, matching the standard Flutter iOS pattern. The generated `AppDelegate.swift` includes this — ensure it's preserved.
- **No `info.plist` changes needed**: The native logging APIs (`NSLog`, `os_log`) don't require any special permissions or entitlements.
- **Xcode project editing**: Prefer using `flutter create` to generate the project (which handles pbxproj correctly), then manually add only the `NativeLogDemo.swift` file reference. Opening the project in Xcode to add the file is the safest approach for the pbxproj edit.
