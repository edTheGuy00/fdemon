// ignore_for_file: dangling_library_doc_comments

/// Synchronous error triggers for Flutter Demon testing.
///
/// These functions intentionally trigger various Dart exceptions
/// to test error highlighting and stack trace display.

// ignore_for_file: only_throw_errors, avoid_print

/// Triggers a null check operator error.
void triggerNullError() {
  String? nullableString;
  // ignore: unnecessary_non_null_assertion
  print(nullableString!.length);
}

/// Triggers a RangeError by accessing an invalid index.
void triggerRangeError() {
  List<int> list = [1, 2, 3];
  print(list[10]);
}

/// Triggers a TypeError by invalid cast.
void triggerTypeError() {
  dynamic value = 'not an int';
  // ignore: unnecessary_cast
  int number = value as int;
  print(number);
}

/// Triggers an AssertionError.
void triggerAssertionError() {
  assert(1 == 2, 'This assertion will fail');
}

/// Triggers an IntegerDivisionByZeroException.
void triggerDivisionByZero() {
  int a = 42;
  int b = 0;
  print(a ~/ b);
}

/// Triggers a FormatException.
void triggerFormatException() {
  int.parse('not a number');
}

/// Triggers a StateError (no element).
void triggerStateError() {
  List<int> emptyList = [];
  print(emptyList.first);
}

/// Triggers an ArgumentError with details.
void triggerArgumentError() {
  throw ArgumentError.value(-1, 'count', 'Must be non-negative');
}

/// Triggers an UnsupportedError.
void triggerUnsupportedError() {
  throw UnsupportedError('This operation is not supported');
}

/// Triggers a custom exception with message.
void triggerCustomException() {
  throw Exception('Custom exception message for testing');
}

/// Triggers an error with a very long message.
void triggerLongErrorMessage() {
  throw Exception(
    'This is a very long error message that is designed to test how the '
    'log viewer handles lengthy error descriptions. It should wrap properly '
    'and remain readable even when the message spans multiple lines. '
    'Additional context: The quick brown fox jumps over the lazy dog. '
    'Lorem ipsum dolor sit amet, consectetur adipiscing elit.',
  );
}
