# Flutter Demon — Example App 1: Dart Logging & Errors

A sample Flutter app for testing fdemon's log capture, filtering, and error
display. This app is **Dart-only** — no native platform code — making it the
ideal starting point to verify log parsing, level colours, and stack trace
rendering.

## What it demonstrates

**Built-in Dart logging**
- `print()` / `debugPrint()` — plain stdout capture
- `dart:developer log()` — structured logging with named levels (debug /
  info / warning / error / shout) and metadata (sequenceNumber, time)

**`logger` package**
- All log levels: trace, debug, info, warning, error, fatal
- Exception objects with attached stack traces
- Structured data objects
- Multi-line messages
- Custom printers (SimplePrinter, no-stack-trace mode)

**`talker` package**
- All severity levels including `critical` and `verbose`
- Exception and Error wrapping
- Typed log records
- HTTP and BLoC lifecycle simulations
- In-app `TalkerScreen` overlay (tap the bug icon in the app bar)

**Error scenarios**
- Sync errors: NullPointerException, RangeError, TypeError, FormatException,
  StateError, ArgumentError, UnsupportedError, custom exceptions, long messages
- Async errors: simple, nested (3 levels), timeout, multiple suspension
  points, uncaught async, stream errors
- Stack traces: 10 / 20 / 50 frame depth, mixed closures, async deep traces

**Performance / ring buffer**
- Spam log buttons: 10 / 50 / 100 mixed logs in a single tap to validate
  fdemon's ring buffer under high throughput

**Environment check**
- Prints Flutter / Dart / platform environment variables to the log view

## How to run

```bash
cd example/app1
fdemon
# or from the repo root:
cargo run -- example/app1
```

Select a connected device or simulator when prompted, then tap buttons in
the app to trigger log output. There are no native platform logs in this
app — all output flows through Flutter's `--machine` stream.
