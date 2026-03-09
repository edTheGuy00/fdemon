## Task: Add Workaround Comment to Zed Generated Config

**Objective**: Add a comment in the Zed `debug.json` generator explaining that `"Delve"` (a Go debugger adapter) is used as a workaround because Zed does not yet have a native Dart/Flutter adapter type.

**Depends on**: None

**Severity**: Minor

### Scope

- `crates/fdemon-app/src/ide_config/zed.rs`: Add explanation to the generated JSON or doc comment at `fdemon_entry()` (~line 44)

### Details

**Current code** (`zed.rs:44`):
```rust
"adapter": "Delve",
```

JSON does not support comments, so the workaround explanation should be:
1. Added as an expanded doc comment on `fdemon_entry()` explaining the rationale
2. Optionally, add a `"_comment"` field to the generated JSON (common pattern in JSON configs):
   ```json
   "_comment": "Uses Delve adapter as a workaround — Zed has no native Dart/Flutter DAP adapter"
   ```

**Recommended approach:** Expand the existing doc comment on `fdemon_entry()` which already mentions "Uses the `Delve` adapter — one of the adapters Zed's debug panel recognises". Add a note that this is a workaround and may break if Zed validates adapter types in the future.

### Acceptance Criteria

1. The Delve workaround is documented in the source code
2. Existing Zed tests pass unchanged (or are updated if a `_comment` field is added)

### Testing

- If adding a `_comment` JSON field, update any tests that assert on the generated JSON structure.

---

## Completion Summary

**Status:** Not Started
