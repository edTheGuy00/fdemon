## Task: Implement Logpoints

**Objective**: Implement logpoint support so breakpoints with a `logMessage` field emit a DAP `output` event instead of pausing execution. Logpoints are breakpoints that log a message (with optional `{expression}` interpolation) and auto-resume. The capability `supportsLogPoints: true` is already advertised.

**Depends on**: 01-wire-debug-event-channel

**Estimated Time**: 3–4 hours

### Scope

- `crates/fdemon-dap/src/adapter/breakpoints.rs`: Store `log_message` on `TrackedBreakpoint`
- `crates/fdemon-dap/src/adapter/mod.rs`: On `PauseBreakpoint` for a logpoint, evaluate message, emit `output`, and auto-resume

### Details

#### How Logpoints Work

1. IDE sends `setBreakpoints` with `SourceBreakpoint` entries containing `log_message` field
2. Adapter sets a normal VM breakpoint (VM doesn't know about logpoints)
3. When the breakpoint fires (`PauseBreakpoint`):
   a. Check if `log_message` is set on the tracked breakpoint
   b. If yes: interpolate `{expression}` placeholders by evaluating each in the current frame
   c. Emit a DAP `output` event with the interpolated message
   d. Auto-resume the isolate
   e. Do NOT emit a `stopped` event
4. If `log_message` is not set: normal breakpoint behavior

#### Log Message Interpolation

The `logMessage` string may contain `{expression}` syntax:
- `"Value of x: {x}"` → evaluates `x` and interpolates
- `"Point: ({point.x}, {point.y})"` → evaluates `point.x` and `point.y`
- `"No interpolation here"` → literal string, no evaluation

```rust
/// Parse logpoint message template and extract expressions.
/// Returns pairs of (literal_text, expression) where expression is None for the final segment.
fn parse_log_message(template: &str) -> Vec<LogSegment> {
    let mut segments = Vec::new();
    let mut remaining = template;

    while let Some(open) = remaining.find('{') {
        let literal = &remaining[..open];
        if let Some(close) = remaining[open..].find('}') {
            let expr = &remaining[open + 1..open + close];
            segments.push(LogSegment::Literal(literal.to_string()));
            segments.push(LogSegment::Expression(expr.to_string()));
            remaining = &remaining[open + close + 1..];
        } else {
            // Unmatched brace — treat rest as literal
            break;
        }
    }
    if !remaining.is_empty() {
        segments.push(LogSegment::Literal(remaining.to_string()));
    }
    segments
}

enum LogSegment {
    Literal(String),
    Expression(String),
}
```

#### Output Event

```json
{
  "type": "event",
  "event": "output",
  "body": {
    "category": "console",
    "output": "Value of x: 42\n",
    "source": {
      "name": "main.dart",
      "path": "/path/to/main.dart"
    },
    "line": 25
  }
}
```

- `category: "console"` — logpoints show in the debug console
- Include source and line from the breakpoint location
- Always append `\n` to output

#### Combined with Conditions

A breakpoint can have both `condition` AND `logMessage`. In this case:
1. Evaluate condition first
2. If truthy: evaluate and emit log message, auto-resume
3. If falsy: silently resume (no log output)

### Acceptance Criteria

1. Setting a breakpoint with `logMessage: "x = {x}"` logs the message to debug console on each hit
2. `{expression}` placeholders are correctly interpolated
3. Logpoints do NOT pause execution
4. Combined condition + logMessage works (condition gates the log)
5. Expression evaluation errors in interpolation produce `<error>` in output
6. Source location included in output event
7. All existing tests pass
8. 12+ new unit tests

### Testing

```rust
#[test]
fn test_parse_log_message_no_expressions() {
    let segments = parse_log_message("Hello world");
    assert_eq!(segments.len(), 1);
    assert!(matches!(&segments[0], LogSegment::Literal(s) if s == "Hello world"));
}

#[test]
fn test_parse_log_message_with_expression() {
    let segments = parse_log_message("x = {x}");
    assert_eq!(segments.len(), 2);
    assert!(matches!(&segments[0], LogSegment::Literal(s) if s == "x = "));
    assert!(matches!(&segments[1], LogSegment::Expression(s) if s == "x"));
}

#[test]
fn test_parse_log_message_multiple_expressions() {
    let segments = parse_log_message("({a}, {b})");
    // Verify: Literal("("), Expr("a"), Literal(", "), Expr("b"), Literal(")")
}

#[test]
fn test_parse_log_message_unmatched_brace() {
    let segments = parse_log_message("unclosed {brace");
    // Verify: treated as literal
}

#[tokio::test]
async fn test_logpoint_emits_output_and_resumes() {
    // Set up adapter with mock backend
    // Set breakpoint with log_message = "x = {x}"
    // Simulate PauseBreakpoint
    // Mock evaluateInFrame("x") → "42"
    // Verify: output event emitted with "x = 42\n"
    // Verify: resume() called
    // Verify: no stopped event
}
```

### Notes

- `SourceBreakpoint.log_message` field already exists in `protocol/types.rs:442`. The adapter just needs to read and store it during `setBreakpoints`.
- Performance: logpoints add one round-trip per `{expression}` per hit. For hot code paths, this could be noticeable. Document this tradeoff.
- Escaped braces: The DAP spec does not define an escape mechanism for literal `{` in logpoints. Most adapters don't support `\{`. Follow suit — any `{` starts an expression until the matching `}`.
