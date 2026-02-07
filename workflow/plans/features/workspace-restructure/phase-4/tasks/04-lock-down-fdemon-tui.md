## Task: Lock Down fdemon-tui Public API

**Objective**: Define a clean public API for `fdemon-tui` by making internal modules (event polling, layout, terminal setup, startup logic) `pub(crate)`. The public API should be the entry points `run_with_project()` and `select_project()`, plus widget types if needed by the binary crate.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-tui/src/lib.rs`: Change module visibility
- Individual module files: Review item-level visibility

### Details

#### 1. Make Internal Modules `pub(crate)`

Several modules in `fdemon-tui` are only used by the TUI runner internally. They should not be part of the crate's public API.

**In `lib.rs`**, change module visibility:

| Module | Current | New | Reason |
|--------|---------|-----|--------|
| `event` | `pub mod` | `pub(crate) mod` | Terminal event polling -- only used by `runner.rs` |
| `layout` | `pub mod` | `pub(crate) mod` | Layout calculations -- only used by `render/` |
| `terminal` | `pub mod` | `pub(crate) mod` | Terminal init/cleanup -- only used by `runner.rs` |
| `startup` | `pub mod` | `pub(crate) mod` | TUI startup logic -- only used by `runner.rs` |
| `render` | `pub mod` | `pub(crate) mod` | Rendering -- only used by `runner.rs` |

Keep these as `pub mod`:
- `runner` -- contains the `run_with_project()` entry point (re-exported at crate root)
- `selector` -- contains `select_project()` entry point (re-exported at crate root)
- `widgets` -- widget types may be useful for the binary crate or future extension

#### 2. Verify Binary Crate Usage

Before making changes, check what the binary crate (`src/main.rs` and `src/headless/`) imports from `fdemon-tui`:

```bash
grep -r "fdemon_tui::" src/
```

Expected: only `fdemon_tui::run_with_project` and `fdemon_tui::select_project` (or `fdemon_tui::SelectionResult`). If any other modules are imported, adjust the plan.

#### 3. Review Widget Module Visibility

The `widgets` module re-exports widget structs that are rendered internally. Check whether any of these are used by the binary crate:

```bash
grep -r "fdemon_tui::widgets" src/
```

If none are used externally, `widgets` could also become `pub(crate)`. However, keeping it `pub` is safer for potential future use (custom widget composition in pro repo).

#### 4. Review Individual Item Visibility

Within the modules that stay `pub`, review item-level visibility:

**`runner.rs`:**
- `run_with_project()` -- keep `pub` (entry point)
- `run()` -- keep `pub` (testing entry point)
- `run_loop()` -- currently `fn` (private). Correct.

**`selector.rs`:**
- `select_project()` -- keep `pub`
- `SelectionResult` -- keep `pub`
- Internal helpers -- should be `fn` (private) or `pub(crate)` at most

**`widgets/mod.rs`:**
- Widget struct types -- keep `pub` (they may be useful for custom rendering)
- Internal widget helpers -- should be `pub(crate)` if only used within the module

### Acceptance Criteria

1. `event`, `layout`, `terminal`, `startup`, `render` modules are `pub(crate)`
2. `runner`, `selector`, `widgets` modules remain `pub`
3. Only `run_with_project`, `run`, `select_project`, and `SelectionResult` are accessible from outside `fdemon-tui`
4. Widget types remain accessible through `fdemon_tui::widgets::*` if `widgets` stays `pub`
5. `cargo check -p fdemon-tui` passes
6. `cargo test -p fdemon-tui` passes
7. `cargo check --workspace` passes (binary crate still builds)
8. `cargo test --workspace` passes

### Testing

```bash
# Crate-level verification
cargo check -p fdemon-tui
cargo test -p fdemon-tui

# Binary crate verification (most likely consumer)
cargo check
cargo test

# Full workspace verification
cargo check --workspace
cargo test --workspace
cargo clippy --workspace
```

### Notes

- The `test_utils` module is already behind `#[cfg(test)]` -- no changes needed
- The `widgets` module staying `pub` is intentional: the pro repo may want to compose custom UIs from these widget types
- TUI crate is relatively clean already -- it has a clear entry point pattern. This task mostly formalizes it
- The `render` module's `view()` function is only called from `runner.rs`, so making `render` `pub(crate)` is safe
- `layout::ScreenAreas`, `layout::LayoutMode`, and layout calculation functions become inaccessible externally -- this is correct since only `render/` uses them
