## Task: Add file size guard to entry point discovery

**Objective**: Prevent memory issues when scanning large generated Dart files by skipping files over 1MB in `has_main_function()`.

**Depends on**: None (can be done independently)

### Scope

- `src/core/discovery.rs`: Modify `has_main_function()` to skip large files

### Details

The current `has_main_function()` reads the entire file into memory to check for a `main()` function. Large generated files (e.g., localization, protobuf, code generators) can cause memory issues.

Add a file size check before reading:

```rust
/// Maximum file size to check for main() function (1MB)
const MAX_MAIN_CHECK_FILE_SIZE: u64 = 1024 * 1024;

pub fn has_main_function(path: &Path) -> bool {
    // Skip files that are too large (likely generated code)
    if let Ok(metadata) = fs::metadata(path) {
        if metadata.len() > MAX_MAIN_CHECK_FILE_SIZE {
            tracing::debug!(
                "Skipping large file ({} bytes): {}",
                metadata.len(),
                path.display()
            );
            return false;
        }
    }

    match fs::read_to_string(path) {
        Ok(content) => has_main_function_in_content(&content),
        Err(_) => false,
    }
}
```

### Acceptance Criteria

1. Files larger than 1MB are skipped in `has_main_function()`
2. Debug log message when skipping large files
3. Existing behavior unchanged for normal-sized files
4. Unit test verifies large file skipping
5. Code compiles without warnings

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_has_main_function_skips_large_files() {
        let temp = TempDir::new().unwrap();
        let large_file = temp.path().join("large.dart");

        // Create a file just over 1MB
        let content = format!(
            "void main() {{ print('hello'); }}\n{}",
            "x".repeat(1024 * 1024 + 1)
        );
        fs::write(&large_file, content).unwrap();

        // Should return false because file is too large
        assert!(!has_main_function(&large_file));
    }

    #[test]
    fn test_has_main_function_accepts_normal_files() {
        let temp = TempDir::new().unwrap();
        let normal_file = temp.path().join("main.dart");

        fs::write(&normal_file, "void main() { print('hello'); }").unwrap();

        // Normal sized file should be checked
        assert!(has_main_function(&normal_file));
    }
}
```

### Notes

- 1MB threshold chosen as reasonable cutoff for hand-written Dart entry points
- Entry point files with `main()` are typically small (< 100 lines)
- Generated files (localization, protobuf) can be several MB
- This is a simple, low-risk mitigation for memory issues

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/core/discovery.rs` | Added MAX_MAIN_CHECK_FILE_SIZE constant, file size check in has_main_function(), and unit tests |

### Notable Decisions/Tradeoffs

1. **File size check placement**: Added the size check before reading the file to avoid unnecessary I/O and memory allocation for large files. This ensures we fail fast for large files without consuming resources.
2. **Debug logging**: Used tracing::debug! for logging skipped files, which allows troubleshooting without cluttering normal output.
3. **Error handling for metadata**: Used if let Ok(metadata) pattern to gracefully handle cases where metadata cannot be read, falling through to the file read attempt.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --lib discovery` - Passed (59 tests including 2 new tests)
- `cargo clippy -- -D warnings` - Passed

### Risks/Limitations

1. **No risks identified**: This is a defensive guard that only adds safety. Files under 1MB are processed normally, and large files that would have caused issues are now safely skipped with debug logging.
