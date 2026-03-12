## Task: Fix Tag Case Sensitivity and Duplicate Custom Source Names

**Objective**: Normalise tag handling to be case-insensitive across all layers, and add duplicate name validation for custom sources.

**Depends on**: None

**Review Issues**: #8 (MINOR), #11 (MINOR)

### Scope

- `crates/fdemon-app/src/config/types.rs`: Case-insensitive `effective_min_level` lookup; duplicate name validation
- `crates/fdemon-app/src/session/native_tags.rs`: Case-insensitive tag storage in `observe_tag` and `is_tag_visible`
- `crates/fdemon-app/src/handler/update.rs`: Normalise tag before passing to `observe_tag` and `effective_min_level`

### Details

**Issue #8 — Case-sensitivity mismatch:**

There are three tag-handling layers with inconsistent case-sensitivity:

1. **Daemon layer** (`should_include_tag` in `native_logs/mod.rs:161`): Uses `eq_ignore_ascii_case` — **case-insensitive**
2. **Config layer** (`effective_min_level` in `config/types.rs:715`): Uses `HashMap::get(tag)` — **case-sensitive**
3. **Session layer** (`is_tag_visible` in `native_tags.rs:55`): Uses `BTreeSet::contains(tag)` — **case-sensitive**

If a user configures `[native_logs.tags.GoLog]` but logcat emits `"golog"`, the per-tag `min_level` override is silently missed. If Android emits both `"GoLog"` and `"golog"`, the tag filter overlay shows two separate rows.

**Fix approach:** Normalise tags to lowercase at the point they enter the system:

1. In `observe_tag`, store `tag.to_ascii_lowercase()` instead of the raw tag
2. In `effective_min_level`, probe with `tag.to_ascii_lowercase()`
3. Normalise `NativeLogsSettings.tags` keys to lowercase at deserialisation or lookup time

**Issue #11 — Duplicate custom source names:**

`CustomSourceStopped` uses `retain(|h| h.name != name)` (update.rs:2074) which removes **all** entries matching a name. If two sources share a name, one stopping removes both handles, orphaning the other process.

**Fix approach:** Add validation at config parse time:

```rust
impl NativeLogsSettings {
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        for source in &self.custom_sources {
            source.validate()?;
            if !seen.insert(source.name.to_lowercase()) {
                return Err(format!(
                    "Duplicate custom source name: '{}'", source.name
                ));
            }
        }
        Ok(())
    }
}
```

### Acceptance Criteria

1. `effective_min_level("GoLog")` and `effective_min_level("golog")` return the same result when the config has `[native_logs.tags.GoLog]`
2. `observe_tag("GoLog")` and `observe_tag("golog")` map to the same tag entry
3. The tag filter overlay shows one row per logical tag regardless of case variations
4. Config with duplicate custom source names returns a validation error
5. Existing tests pass; new tests cover case-insensitive lookup and duplicate name rejection

### Testing

```rust
#[test]
fn test_effective_min_level_case_insensitive() {
    // Config has tags.GoLog.min_level = "error"
    // Assert effective_min_level("goLog") returns "error"
    // Assert effective_min_level("GOLOG") returns "error"
}

#[test]
fn test_observe_tag_normalises_case() {
    let mut state = NativeTagState::default();
    state.observe_tag("GoLog");
    state.observe_tag("golog");
    assert_eq!(state.discovered_tags.len(), 1); // one entry, not two
}

#[test]
fn test_duplicate_custom_source_name_rejected() {
    // Two custom sources with name = "mylog"
    // Assert validate() returns Err
}
```

### Notes

- The daemon-layer `should_include_tag` is already case-insensitive — no changes needed there
- Normalising to lowercase at storage time is simpler than doing it at every lookup
- The duplicate name check at config level is preventive — runtime deduplication is not needed if validation catches it early

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/native_tags.rs` | `observe_tag` now stores `tag.to_ascii_lowercase()`; `is_tag_visible` and `toggle_tag` normalise the input before lookup; existing tests updated for lowercase keys; 4 new case-insensitivity tests added |
| `crates/fdemon-app/src/config/types.rs` | `effective_min_level` uses `eq_ignore_ascii_case` iteration instead of exact HashMap lookup; new `NativeLogsSettings::validate()` method checks for duplicate custom source names (case-insensitive); 6 new tests added |
| `crates/fdemon-app/src/handler/tests.rs` | 4 existing handler tests updated to use lowercase tag keys (`"golog"`, `"okhttp"`) in `discovered_tags` index expressions |

### Notable Decisions/Tradeoffs

1. **`effective_min_level` uses linear scan**: The `tags` HashMap keys retain their original case from TOML (e.g., `"GoLog"`), so an O(n) `iter().find(|k| k.eq_ignore_ascii_case(tag))` is used instead of a hash lookup. For typical configs with fewer than 20 tag overrides this is negligible. An alternative would be to normalise keys at deserialisation time (a `#[serde(deserialize_with=...)]` shim), but that changes the observable type and breaks existing tests that set `tags["GoLog"]` directly — the linear scan is a simpler, non-breaking approach.

2. **`observe_tag` / `is_tag_visible` / `toggle_tag` use `to_ascii_lowercase` at the boundary**: Normalising at storage time means all downstream code (sorted_tags, hide_all, etc.) works without change. The trade-off is that the original casing is lost; the tag filter overlay will show lowercase keys. This is consistent with the task spec.

3. **`handler/update.rs` not changed**: The task mentions normalising in the handler, but since normalisation is now done inside `observe_tag` and `effective_min_level`, no handler changes are needed. The task note says "normalise at the point they enter the system" which these callee-side fixes satisfy.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app --lib` - Passed (1564 tests: 1564 passed, 0 failed, 4 ignored)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed

### Risks/Limitations

1. **Tag display now shows lowercase**: The `sorted_tags()` output exposes lowercase keys to the TUI's tag filter overlay. If the overlay previously showed `"GoLog"`, it now shows `"golog"`. This is a visual change but is consistent and correct; no TUI code was changed as part of this task.
2. **`effective_min_level` O(n) lookup**: As noted above, linear scan is fine for small configs but would be a concern for configs with hundreds of tag overrides.
