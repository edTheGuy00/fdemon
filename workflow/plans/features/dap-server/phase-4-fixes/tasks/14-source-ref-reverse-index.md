## Task: Add reverse index to SourceReferenceStore

**Objective**: Replace O(n) linear scan in `get_or_create` with O(1) lookup using a reverse `HashMap<(String, String), i64>`.

**Depends on**: 02-split-adapter-mod

**Severity**: Minor

### Scope

- `crates/fdemon-dap/src/adapter/stack.rs`: Add reverse index to `SourceReferenceStore`

### Details

**Current (O(n) scan):**
```rust
pub struct SourceReferenceStore {
    next_id: i64,
    references: HashMap<i64, SourceRefEntry>,
}

pub fn get_or_create(&mut self, isolate_id: &str, script_id: &str, uri: &str) -> i64 {
    for (&id, entry) in &self.references {
        if entry.script_id == script_id && entry.isolate_id == isolate_id {
            return id;
        }
    }
    // allocate new...
}
```

**Fixed (O(1) lookup):**
```rust
pub struct SourceReferenceStore {
    next_id: i64,
    references: HashMap<i64, SourceRefEntry>,
    reverse: HashMap<(String, String), i64>,  // (isolate_id, script_id) → ref_id
}

pub fn get_or_create(&mut self, isolate_id: &str, script_id: &str, uri: &str) -> i64 {
    let key = (isolate_id.to_string(), script_id.to_string());
    if let Some(&id) = self.reverse.get(&key) {
        return id;
    }
    let id = self.next_id;
    self.next_id += 1;
    self.references.insert(id, SourceRefEntry { ... });
    self.reverse.insert(key, id);
    id
}
```

Also update the `reset()` method to clear `self.reverse`.

### Acceptance Criteria

1. `get_or_create` uses O(1) HashMap lookup instead of linear scan
2. `reset()` clears the reverse index
3. Existing tests pass
4. `cargo test -p fdemon-dap` — Pass

### Notes

- The key type `(String, String)` allocates on each lookup; consider using `(&str, &str)` with `HashMap::get` if Rust's borrow checker allows, or accept the allocation since this is called once per stack frame per stop
