## Task: Add Reverse-Index to SourceReferenceStore

**Objective**: Replace the O(n) linear scan in `SourceReferenceStore::get_or_create` with an O(1) HashMap lookup by adding a reverse-index keyed by `(isolate_id, script_id)`.

**Depends on**: None

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/stack.rs`: Add reverse-index to `SourceReferenceStore`

**Files Read (Dependencies):**
- None

### Details

#### Current State (stack.rs:69–108)

```rust
pub struct SourceReferenceStore {
    next_id: i64,
    references: HashMap<i64, SourceRefEntry>,  // id → entry (forward lookup)
}

pub fn get_or_create(&mut self, isolate_id: &str, script_id: &str, uri: &str) -> i64 {
    // O(n) scan over ALL entries to find a match
    for (&id, entry) in &self.references {
        if entry.script_id == script_id && entry.isolate_id == isolate_id {
            return id;
        }
    }
    // Insert new entry if not found
    ...
}
```

Called at 4 sites in `stack.rs` (lines 529, 545, 672, 688) — during `stackTrace` and `loadedSources` responses. For apps with hundreds of scripts, `loadedSources` becomes O(n^2).

#### The Fix

Add a reverse-index HashMap:

```rust
pub struct SourceReferenceStore {
    next_id: i64,
    references: HashMap<i64, SourceRefEntry>,
    /// Reverse lookup: (isolate_id, script_id) → reference_id
    by_script: HashMap<(String, String), i64>,
}

pub fn get_or_create(&mut self, isolate_id: &str, script_id: &str, uri: &str) -> i64 {
    let key = (isolate_id.to_string(), script_id.to_string());
    if let Some(&id) = self.by_script.get(&key) {
        return id;
    }
    let id = self.next_id;
    self.next_id += 1;
    self.references.insert(id, SourceRefEntry {
        isolate_id: key.0.clone(),
        script_id: key.1.clone(),
        uri: uri.to_string(),
    });
    self.by_script.insert(key, id);
    id
}
```

Also update:
- `SourceReferenceStore::new()` / `Default` — initialize `by_script: HashMap::new()`
- `SourceReferenceStore::clear()` — also clear `by_script`
- Any remove/reset methods — keep both maps in sync

**Optimization note**: The `get_or_create` key construction allocates two `String`s for the lookup even on cache hit. To avoid this, consider using a borrowed-key pattern or storing a separate hash. However, for the expected cardinality (~100–500 scripts), the allocation is negligible and the simpler code is preferable.

### Acceptance Criteria

1. `get_or_create` uses HashMap lookup, not linear scan
2. Forward and reverse maps are always kept in sync
3. `clear()` resets both maps
4. Existing tests pass: `cargo test -p fdemon-dap`
5. `cargo clippy -p fdemon-dap` clean

### Testing

```rust
#[test]
fn test_source_ref_store_get_or_create_returns_same_id() {
    let mut store = SourceReferenceStore::new();
    let id1 = store.get_or_create("iso1", "script1", "dart:core");
    let id2 = store.get_or_create("iso1", "script1", "dart:core");
    assert_eq!(id1, id2);
}

#[test]
fn test_source_ref_store_different_scripts_get_different_ids() {
    let mut store = SourceReferenceStore::new();
    let id1 = store.get_or_create("iso1", "script1", "dart:core");
    let id2 = store.get_or_create("iso1", "script2", "dart:async");
    assert_ne!(id1, id2);
}

#[test]
fn test_source_ref_store_clear_resets_reverse_index() {
    let mut store = SourceReferenceStore::new();
    store.get_or_create("iso1", "script1", "dart:core");
    store.clear();
    // After clear, a new get_or_create should allocate a fresh ID
    let id = store.get_or_create("iso1", "script1", "dart:core");
    assert_eq!(id, 1); // first allocation after reset
}
```

### Notes

- The existing test suite for `SourceReferenceStore` is in `stack.rs` under `#[cfg(test)]`. All existing tests should pass without modification since the external API doesn't change.
- If `get_or_create` is ever called with different `uri` for the same `(isolate_id, script_id)`, the current behavior (returns existing ID, ignores new uri) is preserved.
