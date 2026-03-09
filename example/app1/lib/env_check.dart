/// Utility to check if dart-define-from-file env values are loaded.
///
/// Usage: Call `EnvCheck.printAll()` from main or a button to verify
/// that --dart-define-from-file is working correctly.
class EnvCheck {
  static const apiBaseUrl = String.fromEnvironment('API_BASE_URL');
  static const appEnvironment = String.fromEnvironment('APP_ENVIRONMENT');
  static const enableLogging = String.fromEnvironment('ENABLE_LOGGING');

  static void printAll() {
    print('=== Dart Define Environment Check ===');
    print('API_BASE_URL: "${apiBaseUrl.isEmpty ? "(not set)" : apiBaseUrl}"');
    print(
      'APP_ENVIRONMENT: "${appEnvironment.isEmpty ? "(not set)" : appEnvironment}"',
    );
    print(
      'ENABLE_LOGGING: "${enableLogging.isEmpty ? "(not set)" : enableLogging}"',
    );
    print('=====================================');

    if (apiBaseUrl.isEmpty && appEnvironment.isEmpty && enableLogging.isEmpty) {
      print('WARNING: No env values loaded! --dart-define-from-file may not '
          'be working.');
    } else {
      print('SUCCESS: Env values are loaded correctly.');
    }
  }
}
