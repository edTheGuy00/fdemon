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
