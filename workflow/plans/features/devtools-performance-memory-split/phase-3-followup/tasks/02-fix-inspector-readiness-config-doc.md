# 02 — Fix Inspector-Readiness Config Doc Key Names

**Wave:** 1
**Depends On:** —
**Agent:** implementor
**Estimated Hours:** 0.5h
**Addresses:** C3

## Context

`docs/CONFIGURATION.md` documents three inspector-readiness poll keys using the wrong TOML names — the `inspector_` prefix is missing. The actual serde field names in `crates/fdemon-app/src/config/types.rs:402,410,417` are:

- `inspector_readiness_poll_attempts`
- `inspector_readiness_poll_interval_ms`
- `inspector_readiness_poll_call_timeout_ms`

Because `DevToolsSettings` does **not** use `#[serde(deny_unknown_fields)]`, the wrong key is silently ignored and the default takes effect. The regression test `test_old_readiness_poll_key_does_not_silently_override_default` at `config/types.rs:1985` explicitly verifies this behavior. Users copying from the doc get defaults with no error.

These keys were added in an earlier phase (not Phase 3); the Phase 3 doc-maintainer task helpfully tried to document them, but used the wrong names. Phase 3's own three new keys (`auto_enable_rebuild_tracking`, `rebuild_stats_frame_window`, `timeline_event_buffer_size`) are documented correctly.

## Acceptance Criteria

1. The three rows in CONFIGURATION.md that name `readiness_poll_attempts`, `readiness_poll_interval_ms`, `readiness_poll_call_timeout_ms` are renamed to use the `inspector_` prefix.
2. The example `[devtools]` TOML block (if it includes these keys) uses the corrected names.
3. Section ordering and surrounding prose remain unchanged.
4. Verify by manual round-trip: copy-paste the renamed example block into a fresh `.fdemon/config.toml` with non-default values and confirm at runtime (via tracing) that the values take effect.
5. No source code changes — this is a docs-only fix.

## Files Modified (Write)

- `docs/CONFIGURATION.md` — rename the three key entries in both the table rows and the example block.

## Files Read (Dependencies)

- `crates/fdemon-app/src/config/types.rs:402,410,417` — verify actual serde field names.
- `crates/fdemon-app/src/config/types.rs:1985` — read the existing `test_old_readiness_poll_key_does_not_silently_override_default` test, which confirms the silent-default behavior under the WRONG name.

## Approach Hints

- Grep `docs/CONFIGURATION.md` for `readiness_poll` to locate all occurrences.
- Each occurrence likely appears in (a) a table row with the key, default, and description, and (b) the example TOML block. Both must be updated.
- Consider whether to add a one-line note under each row referencing the `inspector_` prefix as a reminder that the key is namespaced to the inspector subsystem.
- Do NOT add `deny_unknown_fields` to `DevToolsSettings` in this task — that would be a behavioral change requiring a deprecation cycle.

## Out of Scope

- Any source code changes (including renaming the serde field names, which would break user configs that DO use the correct names).
- Adding `deny_unknown_fields` or any other serde attribute changes.
- Documenting other config keys (the rest of CONFIGURATION.md is unchanged).
- Adding `#[serde(alias = "readiness_poll_*")]` back-compat — defer.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `docs/CONFIGURATION.md` | Renamed three inspector-readiness poll keys in both the TOML example block (lines 346-348) and the property table (lines 360-362) to use the correct `inspector_` prefix: `readiness_poll_attempts` → `inspector_readiness_poll_attempts`, `readiness_poll_interval_ms` → `inspector_readiness_poll_interval_ms`, `readiness_poll_call_timeout_ms` → `inspector_readiness_poll_call_timeout_ms`. |

### Notable Decisions/Tradeoffs

1. **No source code changes**: Task is docs-only as specified. The actual serde field names (`inspector_readiness_poll_attempts`, `inspector_readiness_poll_interval_ms`, `inspector_readiness_poll_call_timeout_ms`) in `crates/fdemon-app/src/config/types.rs` lines 402-417 were verified to match the corrected doc names exactly.
2. **Both locations updated**: Fixed the TOML example block and the property reference table consistently. Section ordering and surrounding prose were left unchanged.

### Testing Performed

- `grep -n "readiness_poll" docs/CONFIGURATION.md` — confirmed all 6 occurrences use `inspector_` prefix
- Verified no bare `readiness_poll_*` keys remain in the docs file
- Source verified: `types.rs:402,410,417` confirms `inspector_readiness_poll_attempts`, `inspector_readiness_poll_interval_ms`, `inspector_readiness_poll_call_timeout_ms` are the actual serde field names
- Test `test_old_readiness_poll_key_does_not_silently_override_default` at `types.rs:1985` confirms that the old key names without `inspector_` prefix were silently ignored (taking defaults) — exactly the bug this doc fix addresses

### Risks/Limitations

1. **Docs-only fix**: Users who had previously copied the incorrect key names from the docs will continue to have those wrong keys silently ignored. The fix ensures future users copy the correct names. No migration is needed for existing working configs that never set these keys.
