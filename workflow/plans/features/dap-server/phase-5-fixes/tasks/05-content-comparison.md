## Task: Add Content Comparison Before File Write

**Objective**: Before writing IDE config files, compare the new content against the existing file. If identical, skip the write and return `ConfigAction::Skipped("content unchanged")`. This eliminates unnecessary git diffs and activates the currently dead `Skipped` variant.

**Depends on**: 04-fix-emacs-merge-path (both modify `run_generator()`)

**Severity**: Major

### Scope

- `crates/fdemon-app/src/ide_config/mod.rs`: Add content comparison in `run_generator()` before `std::fs::write()`

### Details

**Current code** (`run_generator`, lines ~138-150):
```rust
let (content, action) = if generator.config_exists(project_root) {
    let existing = std::fs::read_to_string(&config_path)?;
    let merged = generator.merge_config(&existing, port)?;
    (merged, ConfigAction::Updated)
} else {
    let fresh = generator.generate(port, project_root)?;
    (fresh, ConfigAction::Created)
};

// Unconditional write — even when content unchanged
std::fs::write(&config_path, &content)?;
```

**Fixed code:**
```rust
let (content, action) = if generator.config_exists(project_root) {
    let existing = std::fs::read_to_string(&config_path)?;
    let merged = generator.merge_config(&existing, port)?;
    if merged == existing {
        return Ok(Some(IdeConfigResult {
            ide: generator.ide(),
            action: ConfigAction::Skipped("content unchanged".to_string()),
            config_path,
        }));
    }
    (merged, ConfigAction::Updated)
} else {
    let fresh = generator.generate(port, project_root)?;
    (fresh, ConfigAction::Created)
};

std::fs::create_dir_all(config_path.parent().unwrap())?;
std::fs::write(&config_path, &content)?;
```

The key change: after merge, check `merged == existing`. If identical, early-return with `Skipped`. The `Created` path never needs this check (file didn't exist).

### Acceptance Criteria

1. When content is unchanged, `ConfigAction::Skipped("content unchanged")` is returned
2. When content differs, `ConfigAction::Updated` is returned and the file is written
3. When file is new, `ConfigAction::Created` is returned and the file is written
4. `ConfigAction::Skipped` is no longer dead code
5. No unnecessary file system writes (verify via test with `tempdir`)

### Testing

```rust
#[test]
fn test_run_generator_skips_identical_content() {
    let dir = tempdir().unwrap();
    let gen = VSCodeGenerator;

    // First run: creates the file
    let result1 = run_generator(&gen, 12345, dir.path()).unwrap().unwrap();
    assert!(matches!(result1.action, ConfigAction::Created));

    // Second run with same port: content unchanged, should skip
    let result2 = run_generator(&gen, 12345, dir.path()).unwrap().unwrap();
    assert!(matches!(result2.action, ConfigAction::Skipped(_)));

    // Third run with different port: content changed, should update
    let result3 = run_generator(&gen, 54321, dir.path()).unwrap().unwrap();
    assert!(matches!(result3.action, ConfigAction::Updated));
}
```

### Notes

- The `post_write()` hook (used by Neovim for `.nvim-dap.lua`) should also be skipped when content is unchanged. Check whether `post_write` is called after the early return point — it should not be.
- String equality comparison is cheap for these small config files (< 1KB typically).

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/ide_config/mod.rs` | Added content comparison in `run_generator()` after `merge_config` call; added 2 new tests |

### Notable Decisions/Tradeoffs

1. **No `ide` field in early return**: `IdeConfigResult` only has `path` and `action` fields (no `ide` field), so the early return only populates those two. The tracing log before the early return uses `generator.ide_name()` for visibility.

2. **Tracing before early return**: Added a `tracing::info!` call inside the early-return block so skipped operations are still visible in logs. The existing log at the bottom of `run_generator()` only fires for Created/Updated paths (which is correct since Skipped returns early).

3. **`post_write()` is correctly skipped**: Because the early return exits before the `generator.post_write()` call, secondary file writes (e.g. Neovim's `.nvim-dap.lua`) are not triggered when content is unchanged. This is the correct behaviour per the task notes.

4. **mtime test uses 10ms sleep**: A short sleep is needed to make any spurious write detectable via filesystem mtime. This is a well-established pattern for mtime-based tests.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (all crates: 0 failures, 70 ignored)
- `cargo clippy --workspace -- -D warnings` - Passed
- `cargo test -p fdemon-app "test_run_generator"` - Passed (2/2 new tests)

### Risks/Limitations

1. **mtime granularity on some filesystems**: The mtime test sleeps 10ms between write and skip. On filesystems with coarse-grained mtime (e.g. 1-second resolution like HFS+ without `noatime`), the test could report a false pass even if a write did occur within the same second. The test is still valuable as a regression guard on common CI environments (ext4, APFS) which have millisecond or better resolution.
