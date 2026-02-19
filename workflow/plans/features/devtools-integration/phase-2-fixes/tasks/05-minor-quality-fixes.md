## Task: Minor Quality Fixes Across Extensions and Widget Tree

**Objective**: Address all remaining minor and nitpick issues from the Phase 2 review: magic numbers, doc accuracy, serde derives, code cleanup.

**Depends on**: 01-split-extensions-submodules

**Estimated Time**: 1-2 hours

### Scope

- `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` — Magic number constant
- `crates/fdemon-daemon/src/vm_service/extensions/overlays.rs` — Fix doc comment
- `crates/fdemon-core/src/widget_tree.rs` — Serde derives, PartialEq doc, variable binding cleanup, visible_node_count doc

### Details

#### Fix 1: Add `EXTENSION_NOT_AVAILABLE_CODE` Constant (Review Issue #9)

**File**: `extensions/mod.rs`

Replace the magic number 113 in `is_extension_not_available`:

```rust
// Add alongside existing METHOD_NOT_FOUND_CODE:
/// VM Service error code for "Extension not available" (non-standard, used by some implementations).
const EXTENSION_NOT_AVAILABLE_CODE: i32 = 113;

// Update the check:
if error.code == EXTENSION_NOT_AVAILABLE_CODE {
    return true;
}
```

#### Fix 2: Fix `query_all_overlays` Doc Comment (Review Issue #10)

**File**: `extensions/overlays.rs`

The doc says "concurrently" but the implementation is sequential (`.await` between struct fields). Fix the doc:

```rust
/// Query all 4 overlay extensions sequentially and return their states.
///
/// Each overlay that is not available (e.g., in profile mode) is returned as `None`.
/// Errors from individual overlay queries are silently converted to `None` to
/// support partial results in mixed-mode builds.
```

**Note**: Alternatively, implement actual concurrent querying with `tokio::join!`. The doc fix is simpler and correct for now — concurrency can be added as a future optimization if needed.

#### Fix 3: Add `Serialize`/`Deserialize` to Layout Types (Review Issue #11)

**File**: `crates/fdemon-core/src/widget_tree.rs`

Add serde derives to the three types currently missing them:

```rust
// LayoutInfo (line ~159):
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayoutInfo { ... }

// BoxConstraints (line ~182):
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxConstraints { ... }

// WidgetSize (line ~277):
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WidgetSize { ... }
```

This brings them in line with `DiagnosticsNode` and `CreationLocation` which already have serde. These types will be needed for NDJSON serialization in Phase 4.

#### Fix 4: Document `PartialEq` on f64 Fields (Review Issue #12)

**File**: `crates/fdemon-core/src/widget_tree.rs`

Add doc comments to `BoxConstraints` and `WidgetSize` noting the `PartialEq` limitation:

```rust
/// Box constraints describing minimum and maximum width/height.
///
/// # Equality
///
/// `PartialEq` is derived for convenience (primarily test assertions).
/// For production comparisons involving computed values, use the
/// epsilon-based methods ([`is_tight_width`], [`is_tight_height`]) instead
/// of direct `==`, as floating-point arithmetic can produce imprecise results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxConstraints { ... }
```

Add a similar note to `WidgetSize`.

#### Fix 5: Simplify `BoxConstraints::parse` Double Binding (Review Issue #14)

**File**: `crates/fdemon-core/src/widget_tree.rs` (lines ~217-221)

```rust
// Before:
let min_width = parse_constraint_part(w_part, 'w')?;
let (min_width, max_width) = min_width;
let min_height = parse_constraint_part(h_part, 'h')?;
let (min_height, max_height) = min_height;

// After:
let (min_width, max_width) = parse_constraint_part(w_part, 'w')?;
let (min_height, max_height) = parse_constraint_part(h_part, 'h')?;
```

#### Fix 6: Clarify `visible_node_count()` Doc Comment (Review Issue #13)

**File**: `crates/fdemon-core/src/widget_tree.rs`

```rust
/// Count visible nodes in this subtree for display purposes.
///
/// Returns the number of nodes that would be shown in a tree view.
/// Hidden nodes (level = `"hidden"` or `"off"`) and their entire subtrees
/// are excluded — visible children of a hidden parent are NOT counted.
///
/// Note: Flutter widget trees rarely exceed ~100 levels deep, so the
/// recursive approach is safe in practice.
```

### Acceptance Criteria

1. Magic number `113` replaced with `EXTENSION_NOT_AVAILABLE_CODE` constant
2. `query_all_overlays` doc says "sequentially" not "concurrently"
3. `LayoutInfo`, `BoxConstraints`, `WidgetSize` all derive `Serialize, Deserialize`
4. `BoxConstraints` and `WidgetSize` have doc comments noting `PartialEq` limitation
5. `BoxConstraints::parse` uses single-line destructuring (no double binding)
6. `visible_node_count` doc explicitly describes hidden-parent subtree exclusion behavior
7. All existing tests pass
8. `cargo clippy --workspace -- -D warnings` clean

### Testing

No new tests needed — these are doc, naming, and derive changes. Existing tests validate behavior.

Adding serde derives may require checking that `serde` is already a dependency of `fdemon-core` (it is — `DiagnosticsNode` already uses it). Verify with:

```bash
cargo fmt --all && cargo check --workspace && cargo test --lib && cargo clippy --workspace -- -D warnings
```

### Notes

- These are all low-risk, mechanical changes. They can be done quickly after task 01 establishes the new file structure.
- The serde derive addition (Fix 3) is the most impactful — it adds serialization capability that Phase 4 may need. If any field names need `#[serde(rename = "...")]` annotations, follow the pattern used by `DiagnosticsNode` (which uses `#[serde(rename_all = "camelCase")]`). For `LayoutInfo`, `BoxConstraints`, and `WidgetSize`, snake_case field names are fine since these types are constructed locally, not deserialized from Flutter JSON.
- Fix 2 (query_all_overlays doc) is the simplest — just changing one word. If you prefer to implement actual concurrency with `tokio::join!`, that's fine too, but it changes behavior and requires testing.
