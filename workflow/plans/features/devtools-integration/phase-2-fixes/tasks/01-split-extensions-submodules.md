## Task: Split `extensions.rs` into Submodules

**Objective**: Split the 1955-line `extensions.rs` file into a directory-based module with 5 focused submodules, bringing each file under the 500-line CODE_STANDARDS.md limit.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-daemon/src/vm_service/extensions.rs` — **DELETE** (replaced by directory module)
- `crates/fdemon-daemon/src/vm_service/extensions/` — **NEW** directory
  - `mod.rs` — Re-exports, `ext` constants module, shared parsing helpers, `build_extension_params`
  - `overlays.rs` — `DebugOverlayState`, toggle functions, query/flip helpers
  - `inspector.rs` — `ObjectGroupManager`, `WidgetInspector`, widget tree RPC wrappers
  - `layout.rs` — Layout explorer RPC wrappers, layout info parsing
  - `dumps.rs` — `DebugDumpKind`, debug dump RPC wrappers
- `crates/fdemon-daemon/src/vm_service/mod.rs` — Update re-exports (paths change but names stay same)

### Details

This is a **pure refactor** — no logic changes, no API changes, no new functionality. Every public symbol must remain accessible at the same path from `vm_service/mod.rs`.

#### Target Module Structure

```
vm_service/extensions/
├── mod.rs          (~250 lines prod + ~150 lines tests)
├── overlays.rs     (~155 lines prod + ~100 lines tests)
├── inspector.rs    (~360 lines prod + ~250 lines tests)
├── layout.rs       (~170 lines prod + ~200 lines tests)
└── dumps.rs        (~120 lines prod + ~100 lines tests)
```

#### What Goes Where

**`mod.rs`** (shared infrastructure + re-exports):
- Lines 1-30: Module doc + imports
- Lines 38-85: `pub mod ext` (all 13 extension name constants)
- Lines 99-120: `parse_bool_extension_response`, `parse_data_extension_response`
- Lines 130-164: `METHOD_NOT_FOUND_CODE`, `is_extension_not_available`
- Lines 608-629: `parse_diagnostics_node_response`, `parse_optional_diagnostics_node_response`
- Lines 1040-1052: `build_extension_params` (`pub(super)`)
- Re-export `pub use` for all public items from submodules
- Tests: lines 1065-1101 (parse_bool tests), 1104-1130 (parse_data tests), 1133-1206 (is_extension tests, ext constants tests), 1247-1311 (second parse_bool block, build_extension_params tests)

**`overlays.rs`** (debug overlay toggles):
- Lines 305-314: `DebugOverlayState` struct
- Lines 331-470: `toggle_bool_extension`, `repaint_rainbow`, `debug_paint`, `performance_overlay`, `widget_inspector`, `query_all_overlays`, `flip_overlay`
- Tests: lines 1209-1243 (DebugOverlayState tests)
- Imports from `super`: `ext`, `parse_bool_extension_response`, `super::client::VmServiceClient`

**`inspector.rs`** (widget inspector + object groups):
- Lines 199-293: `ObjectGroupManager` struct + all methods
- Lines 649-741: `get_root_widget_tree`, `get_details_subtree`, `get_selected_widget`
- Lines 941-1031: `WidgetInspector` struct + all methods
- Tests: lines 1411-1621 (parse_diagnostics_node tests — these test `parse_diagnostics_node_response` but belong with inspector semantically; alternatively keep in mod.rs), 1902-1923 (inspector contract test)
- Imports from `super`: `ext`, `parse_diagnostics_node_response`, `parse_optional_diagnostics_node_response`, `build_extension_params`, `super::client::VmServiceClient`

**`layout.rs`** (layout explorer):
- Lines 766-910: `get_layout_explorer_node`, `extract_layout_info`, `parse_widget_size` (private), `extract_layout_tree`, `fetch_layout_data`
- Tests: lines 1624-1953 (extract_layout_info, parse_widget_size, extract_layout_tree, layout contract tests)
- Imports from `super`: `ext`, `parse_diagnostics_node_response`, `super::client::VmServiceClient`
- Imports from `fdemon_core::widget_tree`: `BoxConstraints`, `DiagnosticsNode`, `LayoutInfo`, `WidgetSize`

**`dumps.rs`** (debug dump extensions):
- Lines 481-591: `DebugDumpKind` enum + `debug_dump_app`, `debug_dump_render_tree`, `debug_dump_layer_tree`, `debug_dump`
- Tests: lines 1314-1408 (DebugDumpKind tests, parse_data dump tests)
- Imports from `super`: `ext`, `parse_data_extension_response`, `super::client::VmServiceClient`

#### Key Constraints

1. **`build_extension_params` must stay in `mod.rs`** — It is `pub(super)` and imported by `client.rs` as `super::extensions::build_extension_params`. When `extensions.rs` becomes `extensions/mod.rs`, the path resolves unchanged.

2. **`parse_diagnostics_node_response` must stay in `mod.rs`** — It is a cross-cutting dependency used by both `inspector.rs` and `layout.rs`.

3. **`vm_service/mod.rs` re-exports must not change names** — The 27 re-exports from `extensions::*` on lines 63-71 must keep working. After the split, `extensions/mod.rs` re-exports everything from submodules, so `vm_service::mod.rs` still imports from `extensions::*`.

4. **Inline `super::client::VmServiceClient` references** — The current code uses `super::client::VmServiceClient` path-style. After the split, submodules are one level deeper, so they need `super::super::client::VmServiceClient` or a type alias/re-export in `extensions/mod.rs`.

### Acceptance Criteria

1. `extensions.rs` file is deleted; replaced by `extensions/` directory with 5 files
2. No single file exceeds 500 lines (including tests)
3. All 27 re-exports in `vm_service/mod.rs` still compile and resolve
4. `build_extension_params` is still accessible from `client.rs` via `super::extensions::build_extension_params`
5. All existing tests pass without modification to test logic (only `use` paths may change)
6. `cargo fmt --all` clean
7. `cargo check --workspace` clean
8. `cargo test --lib` — all pass (no regressions from the 446 baseline)
9. `cargo clippy --workspace -- -D warnings` — zero warnings

### Testing

No new tests needed — this is a pure refactor. Verification is:
```bash
cargo fmt --all && cargo check --workspace && cargo test --lib && cargo clippy --workspace -- -D warnings
```

### Notes

- **This task must be completed before tasks 02, 03, and 05** — those tasks modify code that will be in the new submodule files. Doing the split first avoids merge conflicts.
- **Do NOT change any logic** — only move code between files and update `use` paths. Logic fixes are in tasks 02-05.
- The `#[cfg(test)] mod tests` block can either be split per-submodule (preferred) or kept in a single `tests.rs` file. Per-submodule is cleaner because each test group only tests functions from its own submodule.
- When moving code, pay attention to visibility: items that were `pub` in the flat file may need `pub(crate)` or `pub(super)` adjustments. Items called by other submodules must be `pub` in their submodule and re-exported from `mod.rs`.
