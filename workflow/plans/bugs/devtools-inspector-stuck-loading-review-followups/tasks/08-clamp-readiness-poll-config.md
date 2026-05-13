## Task: Clamp `readiness_poll_*` Config Values to Bounded Ranges

**Objective**: Protect against runaway readiness-poll loops by clamping user-supplied config values to reasonable bounds. A typo (`readiness_poll_attempts = 4294967295`) would otherwise saturate the Tokio runtime for up to `inspector_fetch_timeout_secs` seconds.

**Depends on**: 07-use-record-fetch-start-at-auto-fetch-sites (both write `handler/devtools/mod.rs` — schedule sequentially)

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/mod.rs` — apply clamping at the three dispatch sites where `state.settings.devtools.*` is read into `ReadinessPollConfig`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/config/types.rs` — confirm types of the three config keys (`u32`, `u64`, `u64`)

### Details

Per the existing defensive pattern (`fetch_timeout_secs.max(5)`), clamp the three new keys at config-application time. Choose bounds that are generous enough to permit experimentation but tight enough to bound resource use.

**Suggested bounds:**

| Key | Min | Max | Rationale |
|-----|-----|-----|-----------|
| `readiness_poll_attempts` | 0 | 20 | 0 = skip poll entirely; 20 attempts × 5s call timeout × 5s interval = 200s worst case, capped by outer fetch timeout |
| `readiness_poll_interval_ms` | 10 | 5000 | Sub-10ms is pointless; 5s + 20 attempts = 100s outer budget |
| `readiness_poll_call_timeout_ms` | 100 | 10_000 | Sub-100ms is unrealistic for an RPC; 10s per attempt is the upper sane limit |

**Implementation approach: add a private helper in `handler/devtools/mod.rs`:**

```rust
const MAX_READINESS_POLL_ATTEMPTS: u32 = 20;
const MIN_READINESS_POLL_INTERVAL_MS: u64 = 10;
const MAX_READINESS_POLL_INTERVAL_MS: u64 = 5_000;
const MIN_READINESS_POLL_CALL_TIMEOUT_MS: u64 = 100;
const MAX_READINESS_POLL_CALL_TIMEOUT_MS: u64 = 10_000;

fn clamped_readiness_poll_config(settings: &DevToolsSettings) -> (u32, u64, u64) {
    let attempts = settings.readiness_poll_attempts.min(MAX_READINESS_POLL_ATTEMPTS);
    if attempts != settings.readiness_poll_attempts {
        tracing::warn!(
            requested = settings.readiness_poll_attempts,
            clamped_to = attempts,
            "readiness_poll_attempts clamped to bounded range"
        );
    }
    let interval = settings.readiness_poll_interval_ms
        .clamp(MIN_READINESS_POLL_INTERVAL_MS, MAX_READINESS_POLL_INTERVAL_MS);
    if interval != settings.readiness_poll_interval_ms {
        tracing::warn!(
            requested_ms = settings.readiness_poll_interval_ms,
            clamped_to_ms = interval,
            "readiness_poll_interval_ms clamped to bounded range"
        );
    }
    let timeout = settings.readiness_poll_call_timeout_ms
        .clamp(MIN_READINESS_POLL_CALL_TIMEOUT_MS, MAX_READINESS_POLL_CALL_TIMEOUT_MS);
    if timeout != settings.readiness_poll_call_timeout_ms {
        tracing::warn!(
            requested_ms = settings.readiness_poll_call_timeout_ms,
            clamped_to_ms = timeout,
            "readiness_poll_call_timeout_ms clamped to bounded range"
        );
    }
    (attempts, interval, timeout)
}
```

Apply at all three dispatch sites where `ReadinessPollConfig` values are populated into the action. Example:

```rust
// Before:
let action = UpdateAction::FetchWidgetTree {
    session_id,
    readiness_poll_attempts: state.settings.devtools.readiness_poll_attempts,
    readiness_poll_interval_ms: state.settings.devtools.readiness_poll_interval_ms,
    readiness_poll_call_timeout_ms: state.settings.devtools.readiness_poll_call_timeout_ms,
    // ...
};

// After:
let (attempts, interval, timeout) =
    clamped_readiness_poll_config(&state.settings.devtools);
let action = UpdateAction::FetchWidgetTree {
    session_id,
    readiness_poll_attempts: attempts,
    readiness_poll_interval_ms: interval,
    readiness_poll_call_timeout_ms: timeout,
    // ...
};
```

### Acceptance Criteria

1. Values outside the bounded ranges are clamped to the bound and a `warn!` is emitted.
2. Values within the ranges pass through unchanged with no log noise.
3. Unit tests cover: (a) clamp-on-too-high, (b) clamp-on-too-low (for keys with a min), (c) pass-through-on-normal.
4. The three dispatch sites all use the helper; no raw `state.settings.devtools.readiness_poll_*` reads remain except inside the helper.
5. All CI quality gates pass.

### Testing

```rust
#[test]
fn test_clamped_readiness_poll_attempts_capped_at_max() {
    let mut settings = DevToolsSettings::default();
    settings.readiness_poll_attempts = u32::MAX;
    let (attempts, _, _) = clamped_readiness_poll_config(&settings);
    assert_eq!(attempts, MAX_READINESS_POLL_ATTEMPTS);
}

#[test]
fn test_clamped_readiness_poll_interval_floored_at_min() {
    let mut settings = DevToolsSettings::default();
    settings.readiness_poll_interval_ms = 0;
    let (_, interval, _) = clamped_readiness_poll_config(&settings);
    assert_eq!(interval, MIN_READINESS_POLL_INTERVAL_MS);
}

#[test]
fn test_clamped_readiness_poll_passes_through_normal_values() {
    let mut settings = DevToolsSettings::default();
    settings.readiness_poll_attempts = 3;
    settings.readiness_poll_interval_ms = 200;
    settings.readiness_poll_call_timeout_ms = 1500;
    let (a, i, t) = clamped_readiness_poll_config(&settings);
    assert_eq!((a, i, t), (3, 200, 1500));
}
```

### Notes

- The bounds are conservative defaults. If a user has a documented reason to exceed them, raise the bound rather than disabling the clamp.
- The clamp is applied at the *handler* layer, not at deserialization, so the raw value remains in `DevToolsSettings` for visibility/debugging. The handler is the single point of read.
- After task 09 renames the keys to `inspector_readiness_poll_*`, this helper continues to work — it reads `settings.devtools.<key>` directly. Update field names if/when task 09 lands first.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Added 5 constants, `clamped_readiness_poll_config()` helper (`pub(crate)`), updated 2 `FetchWidgetTree` dispatch sites, added 7 unit tests + 1 grep-lint test |
| `crates/fdemon-app/src/handler/update.rs` | Updated the 3rd `FetchWidgetTree` dispatch site (in `RequestWidgetTree` handler) to use the helper |

### Notable Decisions/Tradeoffs

1. **Third dispatch site was in `update.rs`, not `mod.rs`**: The task's "Files Modified" section listed only `mod.rs`, but a third raw read existed in `handler/update.rs`. Made the helper `pub(crate)` so it could be called from `update.rs` via the already-imported `devtools::` path, satisfying the acceptance criterion that no raw reads remain outside the helper.

2. **Struct init syntax in tests**: Clippy's `field_reassign_with_default` lint required test settings to be constructed using `DevToolsSettings { field: value, ..Default::default() }` rather than post-construction field assignment. Used this pattern throughout the new tests.

3. **Grep-lint test added**: Added `test_lint_no_raw_readiness_poll_reads_at_dispatch_sites` to guard against future regressions where someone bypasses the helper in `mod.rs`.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (2208 fdemon-app unit tests, 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- `cargo test -p fdemon-app --lib "clamped_readiness_poll"` - Passed (7/7 new tests)

### Risks/Limitations

1. **No min-floor on `readiness_poll_attempts`**: The task sets `Max=20` but no lower bound (0 is intentional to skip polling entirely). This matches the spec; 0 is a valid "disable" value.
2. **Lint test covers only `mod.rs`**: The grep-lint in `mod.rs` only checks that file. If someone adds a 4th dispatch site elsewhere (e.g., another handler), the lint won't catch it. The `update.rs` site is covered implicitly by the compilation + test suite verifying correct behavior.
