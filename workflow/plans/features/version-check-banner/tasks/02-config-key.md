## Task: Add `[behavior] version_check` config key

**Objective**: Add a boolean `version_check` field to `BehaviorSettings` (default `true`) so users can opt out of the GitHub version check.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 0.5-1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/config/types.rs`: Add the field at the bottom of the `BehaviorSettings` struct (currently at lines 156-167) and update the `Default` impl at lines 169-174.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/config/types.rs` itself for the existing `default_true` helper convention.

### Details

The current struct (per inspection 2026-05-21):

```rust
pub struct BehaviorSettings {
    #[serde(default = "default_true")]
    pub confirm_quit: bool,
    #[serde(default)]
    pub auto_launch: bool,
}

impl Default for BehaviorSettings {
    fn default() -> Self {
        Self {
            confirm_quit: true,
            auto_launch: false,
        }
    }
}
```

After this task:

```rust
pub struct BehaviorSettings {
    #[serde(default = "default_true")]
    pub confirm_quit: bool,
    #[serde(default)]
    pub auto_launch: bool,
    #[serde(default = "default_true")]
    pub version_check: bool,
}

impl Default for BehaviorSettings {
    fn default() -> Self {
        Self {
            confirm_quit: true,
            auto_launch: false,
            version_check: true,
        }
    }
}
```

**Why `default_true` and not `#[serde(default)]`**: a missing key in an existing `.fdemon/config.toml` should opt the user *in* (the check is harmless and useful), not silently disable a feature they never knew about.

### Acceptance Criteria

1. `cargo build -p fdemon-app` succeeds.
2. `cargo test -p fdemon-app config` passes (existing tests should not regress; the field's default is implicitly tested by any test that constructs `BehaviorSettings::default()`).
3. Loading a `config.toml` with no `[behavior]` table yields `version_check: true`.
4. Loading a `config.toml` with `[behavior]\nversion_check = false` yields `version_check: false`.
5. No call site reads `version_check` yet — that wiring is task 04.

### Testing

Add or extend an existing config-parse test. If a test file like `crates/fdemon-app/src/config/types.rs` (or `config/mod.rs`) has a `behavior_defaults_to_*` test, mirror it:

```rust
#[test]
fn behavior_version_check_defaults_to_true_when_table_missing() {
    let toml = ""; // empty config
    let settings: Settings = toml::from_str(toml).unwrap();
    assert!(settings.behavior.version_check);
}

#[test]
fn behavior_version_check_can_be_opted_out() {
    let toml = "[behavior]\nversion_check = false\n";
    let settings: Settings = toml::from_str(toml).unwrap();
    assert!(!settings.behavior.version_check);
}
```

If a similar test already exists for `confirm_quit`, add the new assertions there instead of a new test, to keep the file lean.

### Notes

- This task is fully orthogonal to tasks 01 and 03 — they touch completely different files (Cargo.toml + lib.rs + new module; vs state + widget + handler).
- Documentation of the new key is deferred to task 05a.

---

## Completion Summary

**Status:** Done
**Branch:** feat/version-check-banner

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/config/types.rs` | Added `version_check: bool` field to `BehaviorSettings` struct with `#[serde(default = "default_true")]`, updated `Default` impl to set `version_check: true`, added two new tests: `behavior_version_check_defaults_to_true_when_table_missing` and `behavior_version_check_can_be_opted_out` |

### Notable Decisions/Tradeoffs

1. **Used `default_true` serde helper (not `#[serde(default)]`)**: Ensures that users with an existing `config.toml` that omits the key are opted-in by default. This matches the task rationale: the version check is harmless and useful, so silent opt-out on upgrade would be surprising.

### Testing Performed

- `cargo build -p fdemon-app` — Passed
- `cargo test -p fdemon-app config` — Passed (539 tests, 0 failed; both new tests confirmed passing)
- `cargo fmt --all -- --check` — Passed
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **No call sites yet**: As specified, no code reads `version_check` yet. Task 04 (spawn-and-wire) is responsible for that wiring.
