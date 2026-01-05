// ignore_for_file: dangling_library_doc_comments

/// Deep stack trace generators for Flutter Demon testing.
///
/// These functions generate deep stack traces to test stack trace
/// rendering, collapsing, and scrolling in the log viewer.

/// Generates a stack trace 10 levels deep.
void deepStackTrace() {
  _level1();
}

void _level1() => _level2();
void _level2() => _level3();
void _level3() => _level4();
void _level4() => _level5();
void _level5() => _level6();
void _level6() => _level7();
void _level7() => _level8();
void _level8() => _level9();
void _level9() => _level10();
void _level10() {
  throw Exception('Deep stack trace error at level 10');
}

/// Generates a very deep stack trace using recursion (20 levels).
void veryDeepStackTrace() {
  _recursiveCall(20);
}

void _recursiveCall(int depth) {
  if (depth <= 0) {
    throw Exception('Very deep stack trace error at depth 20');
  }
  _recursiveCall(depth - 1);
}

/// Generates an extremely deep stack trace (50 levels).
void extremelyDeepStackTrace() {
  _deepRecursive(50);
}

void _deepRecursive(int remaining) {
  if (remaining <= 0) {
    throw Exception('Extremely deep stack trace at depth 50');
  }
  _deepRecursive(remaining - 1);
}

/// Generates a stack trace with mixed named and anonymous functions.
void mixedStackTrace() {
  _namedFunction1(() {
    _namedFunction2(() {
      (() {
        // Anonymous closure
        _namedFunction3(() {
          throw Exception('Mixed stack trace with closures');
        });
      })();
    });
  });
}

void _namedFunction1(void Function() callback) => callback();
void _namedFunction2(void Function() callback) => callback();
void _namedFunction3(void Function() callback) => callback();

/// Generates an async deep stack trace.
Future<void> asyncDeepStackTrace() async {
  await _asyncLevel1();
}

Future<void> _asyncLevel1() async {
  await _asyncLevel2();
}

Future<void> _asyncLevel2() async {
  await _asyncLevel3();
}

Future<void> _asyncLevel3() async {
  await _asyncLevel4();
}

Future<void> _asyncLevel4() async {
  await _asyncLevel5();
}

Future<void> _asyncLevel5() async {
  throw Exception('Async deep stack trace at level 5');
}
