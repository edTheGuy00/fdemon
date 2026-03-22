## Task: Implement breakpointLocations Request

**Objective**: Add the `breakpointLocations` DAP request handler that returns valid breakpoint positions for a given source range. This enables the IDE to show valid breakpoint positions when the user hovers over the editor gutter, and supports "column breakpoints" (multiple breakpoints on one line).

**Depends on**: 02-expand-backend-trait

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/handlers.rs`: Add `breakpointLocations` to dispatch table with handler
- `crates/fdemon-dap/src/protocol/types.rs`: Add `supports_breakpoint_locations_request: Some(true)` to `fdemon_defaults()`

### Details

#### Handler implementation:

```rust
async fn handle_breakpoint_locations(&mut self, request: &DapRequest) -> DapResponse {
    let args = parse_args::<BreakpointLocationsArguments>(request);

    let source_path = args.source.path.as_deref()
        .ok_or("Source path required")?;

    // Convert file path to Dart URI
    let uri = path_to_dart_uri(source_path);

    let isolate_id = self.most_recent_isolate_id()
        .ok_or("No isolate available")?;

    // Find the script ID for this URI
    let scripts = self.backend.get_scripts(&isolate_id).await?;
    let script_id = find_script_id_by_uri(&scripts, &uri)
        .ok_or("Script not found")?;

    // Get source report with PossibleBreakpoints
    let report = self.backend.get_source_report(
        &isolate_id,
        &script_id,
        &["PossibleBreakpoints"],
        None,  // tokenPos — would need line-to-token mapping
        None,  // endTokenPos
    ).await?;

    // Convert source report ranges to DAP BreakpointLocation objects
    let locations = extract_breakpoint_locations(&report, args.line, args.end_line);

    DapResponse::success(request, json!({ "breakpoints": locations }))
}
```

#### Source report parsing:

The VM Service `getSourceReport` with `PossibleBreakpoints` returns:
```json
{
  "ranges": [
    {
      "scriptIndex": 0,
      "startPos": 100,
      "endPos": 200,
      "possibleBreakpoints": [105, 120, 145, 180]
    }
  ],
  "scripts": [{ "id": "scripts/1", "uri": "..." }]
}
```

Each `possibleBreakpoints` entry is a token position. These need to be mapped to line/column numbers. Options:
1. Call `getObject(scriptId)` to get the script with its `tokenPosTable` for position mapping
2. Use a simpler line-range filter if token-to-line mapping is too complex

For the initial implementation, use approach 2: request the full source report and filter entries by the requested line range.

#### Response format:

```json
{
  "breakpoints": [
    { "line": 10, "column": 5 },
    { "line": 10, "column": 25 },
    { "line": 11, "column": 3 }
  ]
}
```

### Acceptance Criteria

1. `breakpointLocations` returns valid positions for a source file
2. Positions are filtered to the requested line range
3. `supportsBreakpointLocationsRequest: true` in capabilities
4. Returns empty array for files with no possible breakpoints
5. 4+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_breakpoint_locations_returns_positions() {
    // MockBackend: get_source_report returns ranges with possibleBreakpoints
    // Verify response contains BreakpointLocation objects with line/column
}

#[tokio::test]
async fn test_breakpoint_locations_empty_for_comment_line() {
    // Source report has no possible breakpoints in requested range
    // Verify empty array returned
}
```

### Notes

- This is a differentiator — neither the Dart DDS adapter nor Dart-Code implement `breakpointLocations`. fdemon will provide better breakpoint placement UX.
- Token position to line/column mapping requires the script's `tokenPosTable`. This is a 2D array where each row is `[line, tokenPos, column, tokenPos, column, ...]`. Parsing this correctly is important for accuracy.
- If the `tokenPosTable` parsing is too complex for this task, return breakpoints at the line level only (no column) as a first pass, and add column-level accuracy in a follow-up.

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/protocol/types.rs` | Added `BreakpointLocationsArguments` and `BreakpointLocation` types; set `supports_breakpoint_locations_request: Some(true)` in `fdemon_defaults()`; updated pre-existing test assertion |
| `crates/fdemon-dap/src/adapter/handlers.rs` | Added `BreakpointLocation` and `BreakpointLocationsArguments` imports; added `"breakpointLocations"` to dispatch table; added `handle_breakpoint_locations` method; added `find_script_id_by_uri`, `build_token_pos_map`, and `extract_breakpoint_locations` free helper functions |
| `crates/fdemon-dap/src/adapter/tests/mod.rs` | Added `mod breakpoint_locations` declaration |
| `crates/fdemon-dap/src/adapter/tests/breakpoint_locations.rs` | New file: 7 unit tests covering all acceptance criteria |

### Notable Decisions/Tradeoffs

1. **Token position mapping is fully implemented**: The `tokenPosTable` parsing (building a `HashMap<tokenPos → (line, col)>`) is included, providing column-level accuracy. Token positions not present in the map are silently skipped rather than falling back to line-level-only positions.

2. **Script not found returns empty, not error**: When the requested source file is not in the isolate's script list, the handler returns `{ "breakpoints": [] }` with a success response. This is intentional — the file may not be loaded yet, and the IDE should not be shown an error in that case.

3. **Isolate selection uses paused-first heuristic**: The handler prefers `most_recent_paused_isolate()` over `primary_isolate_id()` so that in multi-isolate sessions the most relevant context is used.

4. **Sorted and deduped output**: Locations are sorted by `(line, column)` and deduplicated before being returned, ensuring deterministic, clean output for IDEs.

5. **Pre-existing test updated**: `test_capabilities_fdemon_defaults` previously asserted `supports_breakpoint_locations_request.is_none()` as a "not yet implemented" marker. This was updated to `assert_eq!(..., Some(true))` to reflect the implemented state — a required change, not a violation of the additive constraint.

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check --workspace` — Passed
- `cargo test -p fdemon-dap` — Passed (770 tests, including 7 new)
- `cargo test --workspace` — Passed (all crates)
- `cargo clippy --workspace -- -D warnings` — Passed (no warnings)

### Risks/Limitations

1. **Token position mapping requires `tokenPosTable` in the source report**: The VM only includes `tokenPosTable` in the script objects embedded in the source report when the script is fully loaded. If the table is absent, all possible breakpoints for that range are silently excluded from the response (the positions exist in `possibleBreakpoints` but cannot be mapped to line/column).

2. **No token position range filtering**: The implementation requests the full source report without `tokenPos`/`endTokenPos` bounds. This is the approach specified in the task's "Details" section (approach 2). A future optimization could pass token position bounds to reduce the RPC payload size.
