import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// A demo page that triggers native platform logging via method channels.
///
/// Android: calls android.util.Log (NativeDemo, MyPlugin, GoLog, OkHttp tags)
/// macOS:   calls NSLog and os_log (com.example.myplugin, com.example.network subsystems)
///
/// These native logs are invisible to Flutter's --machine output, demonstrating
/// the gap that fdemon's native log capture feature addresses.
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
              _periodicRunning
                  ? 'Stop Periodic Logs'
                  : 'Start Periodic Logs (2s)',
            ),
          ),
        ),
      ],
    );
  }
}
