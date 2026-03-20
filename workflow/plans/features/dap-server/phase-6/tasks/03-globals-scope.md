## Task: Implement Globals Scope

**Objective**: Replace the stub globals scope (currently returns `Vec::new()`) with a real implementation that enumerates library-level static fields via the VM Service. When a user expands the "Globals" scope in the IDE, they should see all top-level variables and static fields from the current frame's library.

**Depends on**: 01-fix-variable-display-bugs, 02-expand-backend-trait

**Estimated Time**: 3–5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/variables.rs`: Replace `ScopeKind::Globals` stub in `get_scope_variables` with real library field enumeration

**Files Read (Dependencies):**
- `crates/fdemon-dap/src/adapter/backend.rs`: `get_isolate`, `get_object` method signatures
- `crates/fdemon-dap/src/adapter/stack.rs`: `FrameStore::lookup_by_index` for isolate_id recovery

### Details

#### Current state (stub):
```rust
ScopeKind::Globals => {
    // Globals are expensive — return empty for now.
    Ok(Vec::new())
}
```

#### Implementation:

1. Look up the `isolate_id` from `frame_store.lookup_by_index(frame_index)`
2. Call `backend.get_stack(isolate_id, Some(frame_index + 1))` to get the frame
3. Extract the frame's library reference: `frame.code.owner` should contain a `LibraryRef` with an `id` field. If `frame.code.owner` is a `ClassRef`, traverse to `owner.library` to get the library.
4. Call `backend.get_object(isolate_id, library_id, None, None)` to get the full `Library` object
5. Read `library.variables` — array of `FieldRef { id, name, ... }`
6. For each field:
   - Call `backend.get_object(isolate_id, field_id, None, None)` to get the `Field` object
   - Read `field.staticValue` — this is an `InstanceRef`
   - If `staticValue` is `null` or absent (uninitialized), show `"<not initialized>"` with `variablesReference: 0`
   - Otherwise, convert via `instance_ref_to_variable(field_name, static_value, isolate_id)`
   - Set `presentationHint.attributes: ["static"]` on each variable
7. Handle `start`/`count` pagination — the caller in `handle_variables` already applies pagination after calling this function

#### Fallback for missing `code.owner`:

If the frame doesn't have a `code.owner` with a library reference (e.g., closure frames, async gap frames), fall back to:
1. Call `backend.get_isolate(isolate_id)` to get the full isolate
2. Use `isolate.rootLib` as the library to enumerate
3. This is the same library the debug console REPL would evaluate against

#### Performance considerations:

- Mark globals scope as `expensive: true` (already done)
- Fetching all fields may require N+1 `get_object` calls (1 for library + N for fields)
- Consider caching the globals result per frame — but since `var_store` resets on resume, this is naturally bounded
- Cap at `MAX_VARIABLES_PER_REQUEST` if the library has many fields

#### Also update `get_root_library_id` in `evaluate.rs`:

Replace the `get_vm()` heuristic at `evaluate.rs:376-400` with:
```rust
let isolate = backend.get_isolate(isolate_id).await?;
let root_lib_id = isolate.get("rootLib")
    .and_then(|lib| lib.get("id"))
    .and_then(|id| id.as_str())
    .ok_or_else(|| "Isolate has no rootLib".to_string())?;
```

### Acceptance Criteria

1. Expanding "Globals" scope in IDE shows library-level static variables
2. Each global variable shows correct type and value
3. Expandable globals (objects, collections) have `variablesReference > 0`
4. Global variables have `presentationHint.attributes: ["static"]`
5. Frame without library context falls back to root library
6. `get_root_library_id` in `evaluate.rs` uses `get_isolate` instead of `get_vm`
7. 12+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_globals_scope_returns_library_fields() {
    // MockBackend: get_stack returns frame with code.owner.id = "libraries/1"
    // MockBackend: get_object("libraries/1") returns Library with variables: [FieldRef...]
    // MockBackend: get_object(field_id) returns Field with staticValue: InstanceRef
    // Call get_scope_variables(0, Globals)
    // Verify non-empty result with correct variable names and values
}

#[tokio::test]
async fn test_globals_scope_fallback_to_root_lib() {
    // MockBackend: get_stack returns frame without code.owner
    // MockBackend: get_isolate returns isolate with rootLib.id
    // Verify globals still returns results from root library
}

#[tokio::test]
async fn test_globals_scope_uninitialized_field() {
    // Field with no staticValue → "<not initialized>" display
}
```

### Notes

- The VM Service `Library` object has `variables`, `functions`, and `classes` arrays. Only `variables` (top-level fields/globals) should be returned in the Globals scope. Functions and classes are not variables.
- Private fields (starting with `_`) should still be shown — the user may want to inspect them. Set `visibility: "private"` in presentation hint.
- `const` fields should have `presentationHint.attributes: ["static", "readOnly", "constant"]`.

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/variables.rs` | Replaced `ScopeKind::Globals` stub with `get_globals_variables` implementation; added `resolve_library_id_for_frame` and `get_root_lib_from_isolate` helper methods |
| `crates/fdemon-dap/src/adapter/evaluate.rs` | Replaced fragile `get_vm()` loop in `get_root_library_id` with direct `get_isolate()` call |
| `crates/fdemon-dap/src/adapter/tests/stack_scopes_variables.rs` | Added 13 new globals-scope tests covering all acceptance criteria |
| `crates/fdemon-dap/src/adapter/tests/production_hardening.rs` | Updated count-capping test to use `VariableRef::Object` instead of `ScopeKind::Globals` (since globals now requires a registered frame) |

### Notable Decisions/Tradeoffs

1. **Library resolution chain**: Frame → `code.owner` (direct Library or ClassRef→library) → fallback to `isolate.rootLib`. This handles closure frames and async gaps gracefully.
2. **`get_root_library_id` migration**: Replaced the `get_vm()` scan loop with `get_isolate()` which is more reliable and was the intended usage once task 02 added the method.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-dap` - Passed (612 tests, 13 new)
- `cargo test --workspace` - Passed
- `cargo clippy --workspace` - Passed

### Risks/Limitations

1. **N+1 queries**: Fetching globals requires 1 `get_object` call for the library + N calls for each field. Performance is bounded by `MAX_VARIABLES_PER_REQUEST` and the scope is already marked `expensive: true`.
