## Task: Deduplicate `clean_jsonc` Parser

**Objective**: Remove the duplicate `clean_jsonc`, `strip_json_comments`, and `strip_trailing_commas` implementations from `config/vscode.rs`. The canonical copy in `ide_config/merge.rs` (already `pub` and re-exported) becomes the single source of truth.

**Depends on**: None

**Severity**: Major

### Scope

- `crates/fdemon-app/src/config/vscode.rs`: Delete the three private JSONC functions (~120 lines), import `clean_jsonc` from `crate::ide_config`
- `crates/fdemon-app/src/ide_config/merge.rs`: Remove the "duplicated from config/vscode.rs" comment (lines 66-68)

### Details

**Current state:**

Two functionally identical implementations exist:
- `config/vscode.rs` lines 251-375 — 3 private functions, called from `parse_launch_json()` only
- `ide_config/merge.rs` lines 55-134 — `clean_jsonc` is `pub`, re-exported via `ide_config/mod.rs:255`

The comment in `merge.rs:66-68` says "duplicated to keep modules independent" but both are in `fdemon-app` — `pub(crate)` resolves this with zero coupling.

**Steps:**

1. In `config/vscode.rs`, delete:
   - `fn clean_jsonc(content: &str) -> String` and its body
   - `fn strip_json_comments(content: &str) -> String` and its body
   - `fn strip_trailing_commas(content: &str) -> String` and its body

2. In `config/vscode.rs`, add import:
   ```rust
   use crate::ide_config::clean_jsonc;
   ```

3. Verify the single call site at `parse_launch_json()` (`config/vscode.rs:94`) compiles without changes.

4. In `ide_config/merge.rs`, remove the comment block at lines 66-68:
   ```
   // ─────────────────────────────────────────────────────────────────
   // Internal helpers (duplicated from config/vscode.rs to keep modules independent)
   // ─────────────────────────────────────────────────────────────────
   ```

5. Handle tests in `config/vscode.rs` that directly call the deleted private functions (`strip_json_comments`, `strip_trailing_commas`) — these tests are already covered by `ide_config/merge.rs`'s own test suite. Remove the duplicate test functions. Keep any tests that exercise `parse_launch_json` end-to-end (they test the integration, not the JSONC parser).

### Acceptance Criteria

1. `clean_jsonc` exists in exactly one location (`ide_config/merge.rs`)
2. `config/vscode.rs` imports and uses the canonical copy
3. No test coverage regression — `cargo test -p fdemon-app` passes with same or higher test count
4. `cargo clippy --workspace -- -D warnings` — Pass

### Testing

- Existing tests in `ide_config/merge.rs` cover all JSONC parsing edge cases
- Existing `parse_launch_json` integration tests in `config/vscode.rs` verify end-to-end behavior
- Run `cargo test -p fdemon-app` to confirm no test failures

### Notes

- The `ide_config/merge.rs` copy is the newer, canonical version with slightly better comments.
- If `config/vscode.rs` has tests that exercise edge cases not covered in `merge.rs`, migrate them before deleting.
- This task pairs well with Task 08 (restrict merge visibility) — after dedup, the re-export can be tightened to `pub(crate)`.

---

## Completion Summary

**Status:** Not Started
