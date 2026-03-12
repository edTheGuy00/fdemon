import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return true
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
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
