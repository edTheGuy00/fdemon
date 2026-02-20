## Task: Phase 4 Review Minor Fixes

**Objective**: Fix three minor issues identified in the Phase 4 code review that were deferred to Phase 5: percent_encode_uri casing, overlay toggle debounce (overlaps with Task 04 — coordinate), and layout panel object ID verification.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-app/src/handler/devtools.rs`: Fix `percent_encode_uri` to use uppercase hex
- `crates/fdemon-app/src/actions.rs`: Verify layout panel uses `value_id` (not `object_id`) for `getLayoutExplorerNode`

### Details

#### 1. Fix `percent_encode_uri` Uppercase Hex

**Issue**: The `percent_encode_uri()` function in `handler/devtools.rs` encodes characters using lowercase hex digits (e.g., `%2f` instead of `%2F`). RFC 3986 Section 2.1 recommends uppercase hex digits.

**Location**: `crates/fdemon-app/src/handler/devtools.rs`, `percent_encode_uri()` function.

**Fix**: Change the format specifier from `{:02x}` to `{:02X}`:

```rust
// Before:
write!(encoded, "%{:02x}", byte).unwrap();

// After:
write!(encoded, "%{:02X}", byte).unwrap();
```

This affects the DevTools browser URL construction. While browsers accept both cases, uppercase is the standard recommendation and matches what other tools produce.

#### 2. Verify Layout Panel Object ID Usage

**Issue**: The Phase 4 review noted that the layout panel may be using `object_id` where `value_id` is expected for the `getLayoutExplorerNode` RPC call.

**Location**: `crates/fdemon-app/src/actions.rs`, in the `FetchLayoutData` action handler.

**Investigation needed**:
1. Read the `FetchLayoutData` action code
2. Check what ID field it passes to `getLayoutExplorerNode`
3. The Flutter DevTools protocol expects `id` to be the `valueId` from the `DiagnosticsNode`, NOT the `objectId`
4. Verify against the `DiagnosticsNode` struct in `fdemon-core/src/widget_tree.rs` — it should have both `value_id` and an object reference

**If the bug exists**: Change the parameter from `object_id` to `value_id`:

```rust
// Before (hypothetical):
params.insert("id".into(), json!(node.object_id));

// After:
params.insert("id".into(), json!(node.value_id));
```

**If `value_id` is already used**: Document this finding in the completion summary — the review concern was speculative and doesn't apply.

#### 3. Overlay Toggle Debounce (Coordination with Task 04)

**Issue**: Debug overlay toggle has no debounce/rate-limit. Rapid key presses fire multiple RPCs.

**Note**: This is the same issue addressed in Task 04 (performance-polish). If Task 04 is implemented first, this item is already resolved. If this task is implemented first, add the debounce logic here and Task 04 can skip the overlay section.

**Coordination**: Whichever task is implemented first handles the overlay debounce. The other task should verify it's done and skip.

If implementing here: see Task 04's "Overlay Toggle Debounce" section for the implementation approach (500ms cooldown with `last_overlay_toggle: Option<Instant>` on `DevToolsViewState`).

### Acceptance Criteria

1. `percent_encode_uri()` produces uppercase hex digits (e.g., `%3A` not `%3a`)
2. Existing tests for URL encoding updated to expect uppercase hex
3. Layout panel's `getLayoutExplorerNode` call uses the correct ID field (`value_id`)
4. If overlay debounce is not yet implemented by Task 04, it is added here
5. No regressions in browser URL construction or layout data fetching

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percent_encode_uri_uppercase_hex() {
        let uri = "ws://127.0.0.1:12345/abc=/ws";
        let encoded = percent_encode_uri(uri);
        // Should contain uppercase %3A, %2F, etc.
        assert!(encoded.contains("%3A")); // colon
        assert!(encoded.contains("%2F")); // forward slash
        assert!(!encoded.contains("%3a")); // no lowercase
        assert!(!encoded.contains("%2f")); // no lowercase
    }

    #[test]
    fn test_layout_fetch_uses_value_id() {
        // This test depends on how FetchLayoutData is structured.
        // Verify the correct ID field is extracted from the DiagnosticsNode
        // and passed as the "id" parameter.
    }
}
```

### Notes

- **The `percent_encode_uri` fix is trivial** — a single character change from `x` to `X` in a format string.
- **The layout `object_id` vs `value_id` issue requires investigation** — read the actual code to determine if the bug exists before fixing. The Phase 4 review flagged it as "needs verification", not a confirmed bug.
- **If both this task and Task 04 are assigned to different implementors**, coordinate on the overlay debounce to avoid conflicts. The TASKS.md dependency graph allows both to run in parallel.
