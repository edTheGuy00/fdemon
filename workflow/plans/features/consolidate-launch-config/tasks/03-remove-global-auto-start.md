# Task 03 — Remove `[behavior] auto_start` global flag

**Agent:** implementor
**Plan:** [../PLAN.md](../PLAN.md) (§6 recommendation)

## Problem (one-liner)

`[behavior] auto_start` in `config.toml` is a redundant global flag. The gate in `crates/fdemon-tui/src/startup.rs:36` is `has_auto_start_config || behavior_auto_start` — so any per-config `auto_start = true` in `launch.toml` already bypasses it. Worse, CONFIGURATION.md wrongly describes the flag as a master toggle. Removing it simplifies the mental model and aligns code with docs.

## Desired behavior

- `BehaviorSettings::auto_start` field removed from the Rust struct.
- Startup gate simplified to `if has_auto_start_config { StartupAction::AutoStart { configs } } else { ... }`.
- Existing `config.toml` files that still have `auto_start = <bool>` under `[behavior]` load without error (serde silently ignores unknown fields — verified on `Settings` / `BehaviorSettings`, neither uses `deny_unknown_fields`).
- A one-time `warn!` is logged during config load when the field is present, telling the user the flag is deprecated and to use per-config `auto_start` in `launch.toml` instead.
- Settings Panel → Project tab no longer shows the row.
- Example project configs are cleaned up.

## Acceptance criteria

1. `BehaviorSettings` no longer has an `auto_start` field.
2. `startup.rs:36` uses only `has_auto_start_config` to gate `StartupAction::AutoStart`.
3. Loading a `config.toml` that contains `[behavior] auto_start = true` or `false`:
   - Succeeds without error.
   - Emits exactly one `warn!("config.toml: [behavior] auto_start is deprecated and has no effect; use per-config auto_start in launch.toml instead")` per process lifetime.
4. `settings_items.rs` project-tab items no longer include `behavior.auto_start`.
5. Save-via-Settings-panel to `config.toml` does not re-add the field.
6. All `example/*/.fdemon/config.toml` files have the `auto_start` line removed from `[behavior]` (they currently have it set to `true` or `false` — see the smoke test from the bug plan).
7. All tests that referenced `BehaviorSettings::auto_start` are updated or deleted.
8. The CHANGELOG for the next release mentions the removal.

## Files modified (write)

- `crates/fdemon-app/src/config/types.rs` — remove the field from `BehaviorSettings` and from its `Default` impl.
- `crates/fdemon-tui/src/startup.rs` — simplify the gate.
- `crates/fdemon-app/src/config/settings.rs` — add the deprecation warning in the config-load path when the raw TOML contains a `[behavior] auto_start` key. May require parsing the raw TOML once to detect the presence of the key (since serde will drop it silently); use a cheap `toml::Value` scan on the file contents and log once.
- `crates/fdemon-app/src/settings_items.rs` — remove the `behavior.auto_start` row from the project-tab items.
- `example/app1/.fdemon/config.toml` — remove `auto_start` from `[behavior]`.
- `example/app2/.fdemon/config.toml` — same.
- `example/app3/.fdemon/config.toml` — same. Also clean up the stale comment block at the top that references the flag.
- `example/app4/.fdemon/config.toml` — same (check if the field is even present first).
- `example/app5/.fdemon/config.toml` — same (check if the field is even present first).
- Any test file under `crates/fdemon-app/` that constructs a `BehaviorSettings` with `auto_start` set.

## Files read (context only)

- `crates/fdemon-app/src/config/settings.rs` — do NOT change the `save_last_selection` / `load_last_selection` / `LastSelection` API surface. Tasks 01 and 02 read these and the TASKS.md overlap matrix assumes they're stable.

## Implementation notes

- **Do not change `save_last_selection`'s signature or behavior.** Tasks 01 and 02 depend on it. If you discover that removing `BehaviorSettings::auto_start` forces a settings-rewrite that touches these functions, stop and escalate.
- **Deprecation warning location:** the cleanest place is wherever the raw TOML is first read from disk (before serde deserialization). A one-time `tracing::warn!` on the first detection is sufficient — no need to track per-file state.
- **Test updates:** search for `behavior.auto_start`, `BehaviorSettings { auto_start`, and `auto_start: true` in contexts that look like `BehaviorSettings` construction. Update or delete the tests. Any test that was asserting "auto_start=true causes auto-launch" should be rewritten to assert per-config auto_start does so.
- **Example cleanup:** in `example/app3/.fdemon/config.toml`, the header comment block explicitly talks about `auto_start is false` (wrong today, as you already saw). Replace the whole comment block with a shorter, accurate description of this file's purpose (profile-mode-lag repro + aggressive DevTools polling).
- **Settings Panel UI:** if `settings_items.rs` also drives a "pretty name" label for the removed row, delete both the label and the SettingItem. No migration UI needed — the row just disappears.

## Verification

```bash
cargo test --workspace
cargo fmt --all
cargo check --workspace
cargo clippy --workspace -- -D warnings

# Manual: old config loads cleanly with deprecation warning
cat > /tmp/test_config.toml <<'EOF'
[behavior]
auto_start = true
confirm_quit = true
EOF
# Copy to a test project's .fdemon/config.toml and run `cargo run -- <project>`
# Expect: exactly one warning line about auto_start being deprecated;
# startup proceeds normally; per-config auto_start still works.
```

## Risks

- If any test in `fdemon-tui` was asserting `behavior.auto_start = true` drives auto-launch (without any per-config auto_start), that test's premise disappears. Delete the test or rewrite it to use per-config auto_start.
- If a user's external tooling reads/writes `[behavior] auto_start` programmatically, their tooling breaks silently (field still "works" serde-wise but has no effect). Accept — this is the deprecation path.
