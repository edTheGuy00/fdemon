/// Test plugin for E2E testing.
///
/// This is a minimal plugin with no platform-specific code.
library test_plugin;

class TestPlugin {
  static String get platformVersion => 'Test Platform 1.0';

  static void logMessage(String message) {
    print('[TEST_PLUGIN] $message');
  }
}
