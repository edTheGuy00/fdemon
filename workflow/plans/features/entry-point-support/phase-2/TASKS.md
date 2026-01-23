# Phase 2: Entry Point Discovery - Task Index

## Overview

Implement automatic discovery of Dart files containing `main()` functions in the `lib/` directory. This enables the fuzzy modal in Phase 3 to present users with valid entry point options.

**Total Tasks:** 2

## Task Dependency Graph

```
┌─────────────────────────────┐
│  01-add-main-function-      │
│  detection                  │
└───────────┬─────────────────┘
            │
            ▼
┌─────────────────────────────┐
│  02-create-discover-        │
│  entry-points               │
└─────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-add-main-function-detection](tasks/01-add-main-function-detection.md) | Done | - | `core/discovery.rs` |
| 2 | [02-create-discover-entry-points](tasks/02-create-discover-entry-points.md) | Done | 1 | `core/discovery.rs` |

## Success Criteria

Phase 2 is complete when:

- [x] `has_main_function()` correctly detects main() in Dart files
- [x] `has_main_function_in_content()` handles various main() declaration styles
- [x] `discover_entry_points()` function implemented
- [x] Correctly finds Dart files with main() in `lib/`
- [x] Handles nested directories within `lib/`
- [x] Returns sorted list with `main.dart` first
- [x] Unit tests cover main() detection patterns
- [x] Unit tests cover discovery edge cases
- [x] `cargo clippy` passes with no warnings

## Verification Commands

```bash
cargo test --lib discovery::tests::test_has_main
cargo test --lib discovery::tests::test_discover_entry_points
cargo clippy -- -D warnings
```

## Notes

- Uses existing `regex` crate (already in dependencies)
- Uses `std::fs` for directory traversal (consistent with existing discovery code)
- Only scans `lib/` directory (not test/, build/, etc.)
- Comment-aware detection is best-effort; edge cases acceptable since users can type custom paths
