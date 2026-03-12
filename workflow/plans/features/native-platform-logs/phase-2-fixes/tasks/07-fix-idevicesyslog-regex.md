## Task: Fix `IDEVICESYSLOG_RE` Regex and `check_idevicesyslog` Availability

**Objective**: Fix two related idevicesyslog issues: (1) the regex fails to parse log lines from devices with spaces in their name, silently dropping all their logs; (2) the availability check falsely reports the tool as unavailable when `--help` exits non-zero.

**Depends on**: None

**Review Issues:** #8 (Minor), #10 (Minor)

### Scope

- `crates/fdemon-daemon/src/native_logs/ios.rs`: Fix `IDEVICESYSLOG_RE` regex (line 49)
- `crates/fdemon-daemon/src/tool_availability.rs`: Fix `check_idevicesyslog` (lines 188-196)

### Details

#### Issue #8: IDEVICESYSLOG_RE regex fails on device names with spaces

The current regex (line 49):
```
r"^(\w{3}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2})\s+\S+\s+(\w+)\(([^)]*)\)\[(\d+)\]\s+<(\w+)>:\s*(.*)$"
```

The `\S+` for the device name field stops at the first space. iOS device names commonly contain spaces: "Ed's iPhone", "My iPad Pro", "iPhone (2)". For these devices, the regex fails to match and `parse_idevicesyslog_line` returns `None` — all log lines are silently dropped.

**Fix:** Replace `\S+` with a non-greedy pattern that matches everything up to the process name. The process name is always followed by `(framework)[pid]`, so we can anchor on `\s+(\w+)\(`:

```
r"^(\w{3}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2})\s+.+?\s+(\w+)\(([^)]*)\)\[(\d+)\]\s+<(\w+)>:\s*(.*)$"
```

Change: `\S+\s+` → `.+?\s+` (non-greedy match for device name including spaces, then whitespace before process name).

**Alternative:** Use a named capture `(?P<device>.+?)` if readability is preferred, but since the device name field is not used in `IdevicesyslogLine` (it's discarded), the non-capturing `.+?` is sufficient.

#### Issue #10: `check_idevicesyslog` relies on `--help` exit code

The current check (lines 188-196):
```rust
async fn check_idevicesyslog() -> bool {
    Command::new("idevicesyslog")
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .inspect_err(|e| tracing::debug!("idevicesyslog check failed: {}", e))
        .unwrap_or(false)
}
```

Some libimobiledevice versions exit non-zero on `--help`, causing a false negative. The same problem was solved for `check_macos_log` in phase-1-fixes by switching to path existence checking.

**Fix:** Use `which`-style lookup or path existence. Since `idevicesyslog` is not at a fixed path (unlike `/usr/bin/log`), check if the command is found by looking at the `Err` variant:

```rust
async fn check_idevicesyslog() -> bool {
    // Check if the binary exists on PATH by attempting to spawn it.
    // Use --help but accept any exit code — we only care that the binary was found.
    match Command::new("idevicesyslog")
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
    {
        Ok(_) => true,  // Binary found — any exit code is fine
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => {
            tracing::debug!("idevicesyslog check failed: {}", e);
            false
        }
    }
}
```

The key change: `Ok(_) => true` instead of `Ok(s) => s.success()`. If the binary was found and executed (any exit code), it's available. Only `ErrorKind::NotFound` means it's not installed.

### Acceptance Criteria

1. `parse_idevicesyslog_line` correctly parses lines from devices with spaces in their name (e.g., "Ed's iPhone")
2. `check_idevicesyslog` returns `true` when `idevicesyslog` is installed, regardless of `--help` exit code
3. Existing tests pass: `cargo test -p fdemon-daemon -- idevicesyslog`
4. New tests for device names with spaces

### Testing

```rust
#[test]
fn test_parse_idevicesyslog_line_device_name_with_spaces() {
    let line = "Mar 15 12:34:56 Ed's iPhone Runner(Flutter)[2037] <Notice>: flutter: Hello";
    let parsed = parse_idevicesyslog_line(line);
    assert!(parsed.is_some());
    let parsed = parsed.unwrap();
    assert_eq!(parsed.process, "Runner");
    assert_eq!(parsed.message, "flutter: Hello");
}

#[test]
fn test_parse_idevicesyslog_line_device_name_with_parentheses() {
    let line = "Mar 15 12:34:56 iPhone (2) Runner(Flutter)[2037] <Notice>: test";
    let parsed = parse_idevicesyslog_line(line);
    assert!(parsed.is_some());
}
```

### Notes

- The regex change from `\S+` to `.+?` is safe because the non-greedy quantifier will stop at the shortest match before `\s+(\w+)\(`, which is the process name pattern. The process name always precedes `(framework)[pid]`, providing a reliable anchor.
- The `check_idevicesyslog` fix follows the pattern established by `check_macos_log` in phase-1-fixes, but adapts for a non-fixed-path binary.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/native_logs/ios.rs` | Changed `\S+\s+` to `.+?\s+` in `IDEVICESYSLOG_RE`; expanded doc comment with new examples; added two new tests for device names with spaces and parentheses |
| `crates/fdemon-daemon/src/tool_availability.rs` | Replaced `Ok(s) => s.success()` pattern with `Ok(_) => true` / `Err(e) if e.kind() == ErrorKind::NotFound => false` / `Err(e) => { debug!(...); false }` in `check_idevicesyslog`; updated doc comment to explain the rationale |

### Notable Decisions/Tradeoffs

1. **Non-greedy `.+?` anchor**: The `.+?` quantifier is safe because the process name is always followed by `(framework)[pid]`, giving a reliable right anchor. The non-greedy match stops at the earliest `\s+(\w+)\(` that completes the full pattern, so device names with arbitrary spaces and punctuation (apostrophes, parentheses) are handled without ambiguity.

2. **Mirroring `check_macos_log` pattern**: Rather than using path-existence checking (which would require knowing the binary's install location), the fix follows the same "accept any exit code, only reject NotFound" logic established for `check_macos_log`. This handles the case where `idevicesyslog --help` exits non-zero on some libimobiledevice versions.

### Testing Performed

- `cargo test -p fdemon-daemon -- idevicesyslog` - Passed (15 tests: 13 pre-existing + 2 new)
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **`test_parse_idevicesyslog_line_device_name_with_parentheses`**: The test input `"iPhone (2) Runner(Flutter)[2037]"` contains parentheses in both the device name field and the framework field. The regex handles this correctly because `[^)]*` inside the framework capture group prevents the framework group from consuming the `)` that closes the device-name parenthesis — the non-greedy device-name match will consume "iPhone (2)" as a unit before anchoring on `Runner(`.
