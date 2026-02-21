## Task: Fix Minor Issues

**Objective**: Address all MINOR issues from the review — small code quality improvements, anti-patterns, and efficiency fixes.

**Depends on**: None
**Severity**: MINOR
**Review ref**: REVIEW.md Issues #9-19

### Scope

Multiple files across 4 crates. Each sub-issue is independent.

### Issue 9: Boolean Passed as String to VM Service

**Files**: `crates/fdemon-daemon/src/vm_service/network.rs` (lines ~65, 364, 432, 521)

**Problem**: `enabled.to_string()` produces `"true"/"false"` strings in a `HashMap<String, String>`, but the Dart VM Service expects JSON booleans.

**Fix**: This requires changing the `call_extension` signature in `vm_service/client.rs` from `HashMap<String, String>` to `HashMap<String, serde_json::Value>`. Then use `serde_json::Value::Bool(enabled)` at all 4 call sites. This is a broader change — if `call_extension` is used by other callers, audit them too.

If the signature change is too invasive, a narrower fix: the VM Service's Dart implementation may accept both string and bool forms. Test whether `"true"` actually works at runtime. If it does, document this as a known quirk with a TODO comment and defer the signature change.

### Issue 10: Unnecessary `.clone()` in Body Text Helpers

**File**: `crates/fdemon-core/src/network.rs` (lines ~117-130)

**Problem**: `body_as_text()` methods clone the body bytes unnecessarily.

**Fix**: Use `std::str::from_utf8(&self.request_body)` returning `Option<&str>` instead of allocating a new `String`. If callers need owned strings, they can call `.to_string()` at the call site.

### Issue 11: Magic Number `10` for Page Step

**File**: `crates/fdemon-app/src/handler/devtools/network.rs` (lines ~104-113)

**Fix**: Define `const NETWORK_PAGE_STEP: usize = 10;` at the top of the file and use it in `handle_network_page_up`/`handle_network_page_down`.

### Issue 12: Magic Number `18` for Label Column Width

**File**: `crates/fdemon-tui/src/widgets/devtools/network/request_details.rs` (line ~125)

**Fix**: Define `const LABEL_COL_WIDTH: u16 = 18;` at the top of the file and use it in the layout constraint.

### Issue 13: O(n) Eviction with `Vec::remove(0)`

**File**: `crates/fdemon-app/src/session/network.rs` (line ~88)

**Problem**: `Vec::remove(0)` shifts every element left — O(n) per removal.

**Fix**: Replace `Vec<HttpProfileEntry>` with `VecDeque<HttpProfileEntry>` for the `entries` field. Use `pop_front()` instead of `remove(0)`. Update all code that indexes into `entries` — `VecDeque` supports random access via `[]` and `.get()` so most usage should be compatible.

Note: This interacts with Issue #8 (selected_index semantics) — coordinate if both are being fixed.

### Issue 14: `filtered_count()` Allocates a Full Vec for `.len()`

**File**: `crates/fdemon-app/src/session/network.rs` (lines ~126-128)

**Problem**: `self.filtered_entries().len()` collects into a Vec just to count.

**Fix**: Add an inline iterator count:

```rust
pub fn filtered_count(&self) -> usize {
    if self.filter.is_empty() {
        return self.entries.len();
    }
    let filter_lower = self.filter.to_lowercase();
    self.entries.iter().filter(|e| {
        // same filter logic as filtered_entries()
        e.uri.to_lowercase().contains(&filter_lower)
            || e.method.to_lowercase().contains(&filter_lower)
            || e.status_code.map_or(false, |s| s.to_string().contains(&filter_lower))
    }).count()
}
```

Consider extracting the filter predicate into a private method to avoid duplicating the logic between `filtered_entries()` and `filtered_count()`.

### Issue 15: `short_content_type` Check Order

**File**: `crates/fdemon-tui/src/widgets/devtools/network/request_table.rs` (lines ~316-334)

**Problem**: `"text"` check comes before `"javascript"` and `"css"`, so `"text/javascript"` matches as `"text"` instead of `"js"`.

**Fix**: Move `"javascript"` and `"css"` checks before the `"text"` check. More specific matches should come first.

### Issue 16: `NetworkDetailTab` in `fdemon-core`

**File**: `crates/fdemon-core/src/network.rs` (lines ~237-244)

**Problem**: `NetworkDetailTab` is a UI concern (sub-tab selection) that doesn't belong in the zero-dependency domain crate.

**Fix**: Move to `crates/fdemon-app/src/session/network.rs` alongside `NetworkState`. Update imports in `fdemon-tui` to reference the new location. Remove from `fdemon-core`'s `lib.rs` re-exports.

### Issue 17: Complex Arc<Mutex<Option<JoinHandle>>> Type

**File**: `crates/fdemon-app/src/handler/devtools/network.rs` (line ~83)

**Fix**: Define a type alias:

```rust
type SharedTaskHandle = std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>;
```

Use it in the function signature and the `Message` variant.

### Issue 18: Manual Cell-by-Cell Background Clear

**File**: `crates/fdemon-tui/src/widgets/devtools/network/mod.rs` (lines ~63-69)

**Problem**: Manual loop to set background on every cell.

**Fix**: Replace with idiomatic ratatui:

```rust
Block::new().style(Style::default().bg(palette::DEEPEST_BG)).render(area, buf);
```

This is not `Clear` (which resets to default) because a custom background color is needed.

### Issue 19: Duplicate Client/Handle Function Variants (~250 lines)

**File**: `crates/fdemon-daemon/src/vm_service/network.rs`

**Problem**: Every function has a `_handle` variant that just unwraps the `Arc<VmServiceClient>`.

**Fix**: Consider a trait abstraction or a helper macro. However, this pattern may be intentional for the crate's API design. If it follows the same pattern as other VM Service modules (performance, inspector), defer this to a cross-cutting refactor task. Mark with a TODO for now.

### Tests

- Verify existing tests still pass after VecDeque migration
- Add test for `short_content_type` with `"text/javascript"` input
- Add test for `filtered_count` matches `filtered_entries().len()`
- Run `cargo clippy` to catch any issues from the refactors

### Verification

```bash
cargo test --workspace
cargo clippy --workspace
cargo fmt --all
```
