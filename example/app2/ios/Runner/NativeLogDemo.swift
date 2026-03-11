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
