library core;

class CoreLogger {
  static void log(String message) {
    print('[CORE] $message');
  }
}

class AppConfig {
  static const String appName = 'Multi-Module Test';
  static const String version = '1.0.0';
}
