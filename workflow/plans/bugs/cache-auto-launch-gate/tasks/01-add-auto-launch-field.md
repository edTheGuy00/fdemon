# Task 01 — Add `[behavior] auto_launch` field

**Plan:** [../BUG.md](../BUG.md) · **Index:** [../TASKS.md](../TASKS.md)
**Agent:** implementor
**Depends on:** —
**Wave:** 1 (parallel with Task 02)

## Goal

Add a new `auto_launch: bool` field (default `false`) to `BehaviorSettings`, plumb it through serde load/save, and surface it as its own row in the Settings Panel's Behavior section. **No behavioral change yet** — the field is read by Tasks 03 and 04. This task is purely the foundation.

## Files Modified (Write)

| File | Change |
|------|--------|
| `crates/fdemon-app/src/config/types.rs` | Add `pub auto_launch: bool` to `BehaviorSettings` (with `#[serde(default)]`); update `Default for BehaviorSettings` to set `auto_launch: false` |
| `crates/fdemon-app/src/config/settings.rs` | Ensure `save_settings` round-trips the new field (it should be automatic via serde, but verify and add a round-trip test) |
| `crates/fdemon-app/src/settings_items.rs` | Add a new `SettingItem` row for `auto_launch` to the Behavior section, alongside `confirm_quit` |

## Files Read (dependency)

— (foundational; no upstream tasks)

## Implementation Notes

### `BehaviorSettings` (types.rs:155-167)

Current shape:
```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BehaviorSettings {
    #[serde(default = "default_true")]
    pub confirm_quit: bool,
}
impl Default for BehaviorSettings {
    fn default() -> Self {
        Self { confirm_quit: true }
    }
}
```

After:
```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BehaviorSettings {
    #[serde(default = "default_true")]
    pub confirm_quit: bool,
    /// When true, fdemon auto-launches the cached `last_device` on startup
    /// (only if `launch.toml` does not have an `auto_start = true` config —
    /// per-config intent always wins). Default false: cache is "remembered
    /// for the dialog" only, not a launch trigger.
    #[serde(default)]
    pub auto_launch: bool,
}
impl Default for BehaviorSettings {
    fn default() -> Self {
        Self { confirm_quit: true, auto_launch: false }
    }
}
```

### Settings Panel row (settings_items.rs)

Find the section that constructs the Behavior tab's `SettingItem` list (look for the existing `confirm_quit` entry — pattern is well-established). Add a new entry with:
- key id: `"behavior.auto_launch"`
- label: `"Auto-launch on cached device"` (or similar)
- accessor: read/write `settings.behavior.auto_launch`
- type: bool toggle

Mirror the structure of the `confirm_quit` row exactly.

### Compatibility

- `BehaviorSettings` does **not** use `#[serde(deny_unknown_fields)]`, so older `config.toml` files lacking `auto_launch` will load as `false` (default) and newer files lacking `confirm_quit` will load as `true`. Verify with a serde-roundtrip unit test.
- The deprecated `[behavior] auto_start` warning emitted in v0.5.0 stays untouched.

## Verification

- `cargo check -p fdemon-app`
- `cargo test -p fdemon-app config::types::tests` — new test:
  ```rust
  #[test]
  fn behavior_settings_auto_launch_defaults_false() {
      let s: BehaviorSettings = toml::from_str("").unwrap();
      assert!(!s.auto_launch);
      assert!(s.confirm_quit);
  }

  #[test]
  fn behavior_settings_auto_launch_round_trips() {
      let toml_in = "auto_launch = true\nconfirm_quit = false";
      let s: BehaviorSettings = toml::from_str(toml_in).unwrap();
      assert!(s.auto_launch);
      let toml_out = toml::to_string(&s).unwrap();
      assert!(toml_out.contains("auto_launch = true"));
  }
  ```
- `cargo test -p fdemon-app settings_items` — verify the new row appears in the Behavior tab's items list.
- `cargo clippy --workspace -- -D warnings`

## Acceptance

- [ ] `BehaviorSettings.auto_launch: bool` exists with `#[serde(default)]` and defaults to `false`.
- [ ] `Default for BehaviorSettings` includes the new field.
- [ ] `save_settings` round-trips `auto_launch` (verified by test).
- [ ] Settings Panel Behavior tab has a row for `auto_launch` styled identically to `confirm_quit`.
- [ ] All existing tests still pass; no behavior change in any other code path.
