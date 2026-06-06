## Task: Fix SDK locator version/channel edge cases and strip_ansi OSC handling (F-PR53-16/17)

**Severity:** LOW (correctness — edge cases)

**Objective**: Three small correctness fixes in SDK detection / diagnostics:
fall back to `flutter.version.json` when the legacy version file is blank, report
the manifest `channel` for git-less installs, and stop `strip_ansi` from dropping
a character on a malformed OSC sequence.

**Depends on**: — (disjoint; safe to parallelize)

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/flutter_sdk/types.rs`
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs`
- `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/flutter_sdk/channel.rs` (`detect_channel`, git-only detection)

### Details

**(a) Blank legacy version file returns `Ok("")`.**
`read_version_file` (`types.rs:261-278`) enters the legacy branch whenever
`<root>/version` or `VERSION` is a file and returns `Ok(content.trim().to_string())`
**without** an emptiness check. A truncated/blank file short-circuits with `Ok("")`
and never reaches the `flutter.version.json` fallback — even though the JSON path
filters empty (`.filter(|s| !s.is_empty())`, line 246). The empty version then
propagates into `FlutterSdk.version`.

**(b) `channel == None` for git-less installs.**
`try_resolve_sdk` computes `channel = detect_channel(&sdk_root)`
(`locator.rs:363`). `detect_channel` (`channel.rs:47-71`) inspects only git state
and returns `None` when there is no `.git` (archive/tarball/wizard-produced
installs). But `flutter.version.json` carries an authoritative `channel` field
(`types.rs` parses only `frameworkVersion`, ignoring `channel`). So modern non-git
installs report `channel = None`, degrading channel-dependent UI/behavior.

**(c) `strip_ansi` OSC consumes a char without checking it is `\`.**
`diagnostics.rs:72-76`: on an inner ESC inside an OSC, the code unconditionally
calls `chars.next()` (assuming an `ESC \` two-char ST) then breaks. A bare inner
ESC not followed by `\` swallows one legitimate following character. Defensive
error-text cleanup only (not on the detection path).

### Proposed Fix

1. In `read_version_file`, trim the legacy content and only return it if non-empty;
   otherwise fall through to `read_framework_version_json(root)`.
2. When `detect_channel` returns `None`, fall back to reading the `channel` field
   from `flutter.version.json` (extend `read_framework_version_json` to also return
   channel, or add a sibling reader) and use it for `FlutterSdk.channel`.
3. In `strip_ansi`, peek before consuming: only `chars.next()` when
   `chars.peek() == Some(&'\\')`; otherwise treat the inner ESC as the terminator
   without eating the next char.

### Acceptance Criteria

1. A root with a blank/whitespace `version` file but a valid `flutter.version.json`
   resolves to the JSON `frameworkVersion`, not `""`.
2. A git-less install whose `flutter.version.json` has `"channel": "stable"` reports
   `FlutterSdk.channel == Some("stable")`; git installs are unaffected.
3. `strip_ansi("\x1b]2;ab\x1bcd")` no longer drops `c`; well-formed `ESC \` ST
   still strips correctly.

### Testing

```rust
// types.rs test module
// - test_read_version_file_blank_legacy_falls_back_to_json.
// locator.rs test module
// - test_channel_from_version_json_for_gitless_install.
// diagnostics.rs test module
// - test_strip_ansi_malformed_osc_keeps_following_char (extend the existing OSC test).
```

### Notes

- File-disjoint from all other tasks → Wave 1 parallel worktree candidate.
- All three are low-severity edge cases; bundled because they share the
  `flutter_sdk/` module and none alters the primary detection chain.
