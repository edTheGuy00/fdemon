## Task: Strip Stale `#[allow(dead_code)]` from `details/*` + Small Cosmetics

**Objective**: Remove `#[allow(dead_code)]` annotations that task 09 of Phase 1 was supposed to delete after wiring the call sites, plus a handful of small cosmetic fixes in the same files.

**Depends on**: —

**Estimated Time**: 0.5 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs`
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/render_object_tab.rs`
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs`
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs`

**Files Read (Dependencies):**
- None.

### Review Items Resolved

- **M6** — stale `#[allow(dead_code)]` annotations across `details/*`
- **m12** — `_tab` misleading underscore prefix on actively-used binding (`details/mod.rs:136`)
- **m14** (in-scope subset) — single-slash `/ ` doc-comment lines in `details/properties_tab.rs:28` and elsewhere in `details/*` if present

### Details

#### Remove `#[allow(dead_code)]` annotations

Phase 1 task 08 added `#[allow(dead_code)]` because task 09 had not yet wired `render_details_panel`. Task 09 completed and removed it from `details/mod.rs`, but the per-tab files still carry stale annotations. Specifically (line numbers approximate):

- `properties_tab.rs`: `MIN_LAYOUT_PREVIEW_HEIGHT` (line 24), `PROPERTY_LIST_HEIGHT` (line 29), `render_properties_tab` (line 40), `render_property_list_placeholder` (line 82).
- `render_object_tab.rs`: `render` (line 13), `render_centered_text` (line 18).
- `flex_explorer_tab.rs`: `render` (line 13), `render_centered_text` (line 18).

Remove each annotation. After removal, `cargo clippy -- -D warnings` must still pass — any function/constant that genuinely becomes a warning means it is unreachable and should be deleted instead.

#### Fix `_tab` rename

In `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs:136`:

```rust
for (i, (label, _tab)) in TAB_LABELS.iter().enumerate() {
    // ...
    let is_active = *_tab == active;
```

Rename `_tab` → `tab`. The underscore prefix conventionally signals "intentionally unused" but the binding is used three lines below. Matches the second loop in the same function which already uses `tab`.

#### Fix single-slash doc lines (in-scope subset)

In `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs:28`, the line beginning with `/ 2 rows: ...` should be `/// 2 rows: ...` (it's part of a doc-comment block).

Scan the four files in scope for any other `/ ` lines that should be `///` or `//` and fix them. Do NOT scan files outside this task's scope — those are handled in task 04 (`tree_panel.rs:152` is in task 04's territory).

### Acceptance Criteria

1. No `#[allow(dead_code)]` annotations remain in the four `details/*` files.
2. `cargo clippy -p fdemon-tui --all-targets -- -D warnings` passes (no new warnings; no dead code surfaces).
3. The `_tab` binding in `details/mod.rs:136` is renamed to `tab`.
4. The single-slash doc-comment line at `properties_tab.rs:28` (and any others in the four scope files) is fixed.
5. `cargo test -p fdemon-tui` passes — existing tests should be unaffected.
6. `cargo fmt --all -- --check` passes.

### Testing

No new tests required. The acceptance is "no new warnings and no test regressions." Run:

```bash
cargo clippy -p fdemon-tui --all-targets -- -D warnings
cargo test -p fdemon-tui
```

### Notes

- If clippy surfaces a genuinely unreachable function after removing an `#[allow(dead_code)]`, **delete the function**, do not re-add the annotation. The review's finding is that the annotations are stale, not that the underlying functions should remain hidden.
- Worktree note: this task is parallel-safe with tasks 01, 02, 04 (no shared write files).
- Phase 2 will rewrite the stub render-object and flex-explorer tab bodies; no need to consolidate their `render_centered_text` helper here (n5 is explicitly won't-fix per PLAN.md).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a4856d2f96afa20a4

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs` | Removed two `#[allow(dead_code)]` annotations from `render` and `render_centered_text` |
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/render_object_tab.rs` | Removed two `#[allow(dead_code)]` annotations from `render` and `render_centered_text` |
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs` | Removed four `#[allow(dead_code)]` annotations from `MIN_LAYOUT_PREVIEW_HEIGHT`, `PROPERTY_LIST_HEIGHT`, `render_properties_tab`, and `render_property_list_placeholder` |
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` | Renamed `_tab` binding to `tab` in the `render_tab_strip` loop (line 136), and updated the usage on the next line from `*_tab` to `*tab` |

### Notable Decisions/Tradeoffs

1. **No dead code surfaced after removal**: Clippy `-D warnings` passed cleanly — all functions were genuinely reachable via `render_details_panel`, confirming the annotations were stale.
2. **Single-slash doc-comment fix**: The `/ 2 rows:` line mentioned in the task was already `/// 2 rows:` in the current branch — no action needed.

### Testing Performed

- `cargo clippy -p fdemon-tui --all-targets -- -D warnings` — Passed (no warnings)
- `cargo test -p fdemon-tui` — Passed (1086 unit tests + 7 doc tests)
- `cargo fmt --all -- --check` — Passed

### Risks/Limitations

None. Pure cosmetic changes — no logic altered.
