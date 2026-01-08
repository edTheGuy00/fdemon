import 'package:flutter_test/flutter_test.dart';
import 'package:test_plugin/test_plugin.dart';

void main() {
  test('platformVersion returns expected string', () {
    expect(TestPlugin.platformVersion, 'Test Platform 1.0');
  });
}
