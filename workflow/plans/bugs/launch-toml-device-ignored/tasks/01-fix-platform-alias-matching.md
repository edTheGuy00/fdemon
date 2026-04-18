# Task 01 — Alias `"macos"` ↔ `"darwin"` in `Device::matches`

**Agent:** implementor
**Worktree:** isolated (no write-file overlap with sibling tasks)
**Depends on:** none
**Plan:** [../BUG.md](../BUG.md)

## Problem

`flutter devices --machine` reports macOS desktop devices with `targetPlatform = "darwin"`, deserialized into `Device.platform = "darwin"` (verified in test `crates/fdemon-daemon/src/devices.rs:388-441`). However, `Device::matches("macos")` only compares against `self.platform` via `starts_with` and against `self.platform_type` via exact match. Since `"darwin".starts_with("macos") == false` and `platform_type` is `None` for `--machine` output, the matcher returns `false` and the user's `device = "macos"` configuration is dropped.

The display layer (`Device::platform_short`, `crates/fdemon-daemon/src/devices.rs:77-88`) already canonicalizes these aliases:

```rust
"macos" | "darwin" => "macOS",
"chrome" | "web-javascript" => "Web",
p if p.starts_with("ios") => "iOS",
p if p.starts_with("android") => "Android",
```

`matches` and `platform_short` should share a single source of truth so the two never drift out of sync.

## Goal

`Device::matches(specifier)` returns `true` whenever the specifier and the device refer to the same logical platform — using the same alias table that `platform_short` already encodes.

## Implementation Notes

1. Extract a small helper inside `crates/fdemon-daemon/src/devices.rs`:
   ```rust
   /// Canonical short platform identifier for a raw `targetPlatform` string.
   /// Returns one of: "ios", "android", "macos", "windows", "linux", "web", "fuchsia",
   /// or the original lowercased platform string if no alias applies.
   fn canonical_platform(platform: &str) -> &str { ... }
   ```
   Keep it module-private. Have both `Device::platform_short` and `Device::matches` consume it (matches compares lowercased strings; `platform_short` maps to the display capitalisation).
2. In `Device::matches`, after the existing id and name checks, replace the `platform.starts_with(spec)` and `platform_type == spec` checks with a single comparison:
   - Compute `device_canon = canonical_platform(&self.platform.to_lowercase())`.
   - Compute `spec_canon = canonical_platform(&spec_lower)`.
   - Return `true` if either `device_canon == spec_canon` or, for backward compatibility with raw flavors like `"android-arm64"`, `self.platform.to_lowercase().starts_with(&spec_lower)` still matches.
   - Continue to honour `platform_type` exact-match if present.
3. Do **not** change the `Device` struct shape, the public `find_device` signature, or the `LaunchConfig` type.

## Acceptance Criteria

- [ ] A new helper canonicalizes raw platform strings; `platform_short` uses it.
- [ ] `Device::matches("macos")` returns `true` for a device with `platform = "darwin"`.
- [ ] `Device::matches("web")` returns `true` for a device with `platform = "web-javascript"`.
- [ ] Existing happy paths still pass: `matches("ios")` for `platform = "ios"`, `matches("android")` for `platform = "android-arm64"`, exact id match, case-insensitive name match.
- [ ] No public API changes outside `devices.rs`.

## Tests to Add (in `crates/fdemon-daemon/src/devices.rs` test module)

| Test name | Scenario |
|-----------|----------|
| `test_matches_macos_specifier_against_darwin_platform` | Device with `platform = "darwin"`, specifier `"macos"` → `true` |
| `test_matches_darwin_specifier_against_macos_alias` | Device with `platform = "macos"`, specifier `"darwin"` → `true` (symmetry) |
| `test_matches_web_specifier_against_web_javascript_platform` | Device with `platform = "web-javascript"`, specifier `"web"` → `true` |
| `test_matches_android_specifier_against_android_arm64_platform` | Device with `platform = "android-arm64"`, specifier `"android"` → `true` (regression guard for existing `starts_with` behavior) |
| `test_matches_unknown_specifier_returns_false` | Device with `platform = "ios"`, specifier `"windows"` → `false` |

## Verification

```bash
cargo fmt -p fdemon-daemon
cargo test -p fdemon-daemon devices
cargo clippy -p fdemon-daemon -- -D warnings
```

## Files

| File | Change |
|------|--------|
| `crates/fdemon-daemon/src/devices.rs` | Add `canonical_platform` helper; rewrite `Device::matches`; refactor `platform_short` to use the helper; add five unit tests |
