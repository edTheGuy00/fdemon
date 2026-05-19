# 03 — Core Parser Hygiene

**Wave:** 1
**Depends On:** —
**Agent:** implementor
**Estimated Hours:** 1.5–2.5h
**Addresses:** H2, L7, L8

## Context

Three independent quality issues in `fdemon-core`'s parsing layer:

- **H2.** `classify_thread` in `crates/fdemon-core/src/timeline.rs:191–203` uses a simple `.contains(".ui")` check, but the module-level docstring (lines 14–23) promises an exclusion guard for `.flutter.test..ui`. The two are contradictory. The test `classify_thread_tester_special_case` passes only because `"io.flutter.test..ui"` happens to contain `".ui"` — accidental, not by design. A reader trusting the docstring is misled.
- **L7.** `parse_vm_timeline` silently defaults missing `ph` and `tid` fields via `unwrap_or` — but it returns errors on missing `name` and `ts`. This asymmetry is undocumented and there's no diagnostic when an event's `tid` is 0 because the field was absent (events appear as `Other` thread without explanation).
- **L8.** `RebuildEventPayload` does not derive `Serialize`/`Deserialize` while all sibling types in `rebuild_stats.rs` do. The struct holds a `HashMap<String, serde_json::Value>` for `new_locations`; `serde_json::Value` implements both traits, so the derive is possible.

All three are in `fdemon-core`, no cross-crate impact.

## Acceptance Criteria

1. **H2 resolved.** `classify_thread` docstring rewritten to accurately describe the implementation:
   - Document that the rule is a simple `.contains(".ui")` / `.contains(".raster")` / `.contains(".platform")` check.
   - Explicitly note that `io.flutter.test..ui` (the single-track Flutter tester case) is classified as `Ui` because its name contains `.ui` — this is intentional, not a bug.
   - Remove the misleading "exclusion guard" language entirely.
   - The classified-thread test continues to pass without modification.
2. **L7 resolved.** In `parse_vm_timeline`:
   - Add a `tracing::debug!` log line when `ph` defaults to `"?"` (with the event's `name` for context).
   - Add a `tracing::debug!` log line when `tid` defaults to `0` (with the event's `name`).
   - Add a doc comment on the function explaining that `ph` and `tid` are tolerated as absent (defensive — Chrome-trace spec allows it) while `name` and `ts` are required and error on absence.
3. **L8 resolved.** `RebuildEventPayload` derives `Serialize, Deserialize`. New test confirms round-trip: `serde_json::to_string(&payload).unwrap()` → `serde_json::from_str(&s).unwrap()` equals the original. Test placed in the existing `#[cfg(test)] mod tests` block in `rebuild_stats.rs`.
4. `cargo fmt --all -- --check && cargo check -p fdemon-core && cargo test -p fdemon-core && cargo clippy -p fdemon-core --all-targets -- -D warnings` all pass.
5. No public API changes other than the new derives — sibling crates compile without modification.

## Files Modified (Write)

- `crates/fdemon-core/src/timeline.rs` — H2 (docstring rewrite) + L7 (debug logs + function doc comment).
- `crates/fdemon-core/src/rebuild_stats.rs` — L8 (Serialize/Deserialize derives on `RebuildEventPayload` + new round-trip test).

## Files Read (Dependencies)

- `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` — read-only sanity check that none of its callers depend on the (incorrect) old docstring.

## Approach Hints

- For H2: the rewritten docstring should be ~5–8 lines: one sentence describing the simple containment check, one sentence explaining the tester case rationale, one paragraph noting that this differs from the upstream DevTools `.flutter.test..ui` exclusion intentionally for fdemon's purposes.
- For L7: use `tracing::debug!` (not `warn!`) — these are expected occurrences with malformed-but-tolerated events, not problems.
- For L8: ensure the derive does not break any existing test that may compare `RebuildEventPayload` structurally. Likely safe since the type is currently `PartialEq, Eq, Debug, Clone`.
- The round-trip test should construct a `RebuildEventPayload` with at least one populated `new_locations` entry containing a `serde_json::Value` of a nested object (the verbatim Phase 3 shape).

## Out of Scope

- Changing `classify_thread` implementation to actually match the upstream exclusion-guard behavior. (The current implementation is correct for fdemon's use case; we're only fixing the docstring.)
- Adding a separate "TimelineParseDiagnostic" surface for L7 — debug-level tracing suffices.
- Refactoring `parse_vm_timeline` into smaller helpers.
- Adding `Serialize` to other types in `rebuild_stats.rs` that don't have it (they all already do per the L8 note).
