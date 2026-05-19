# 07 — Daemon VM Service Hygiene

**Wave:** 2
**Depends On:** —
**Agent:** implementor
**Estimated Hours:** 1–1.5h
**Addresses:** M5, L2, L6

## Context

Three small hygiene items in `fdemon-daemon`:

- **M5.** `enable_frame_tracking` in `crates/fdemon-daemon/src/vm_service/timeline.rs:147` calls `handle.call_extension("ext.flutter.profileWidgetBuilds", ...)` using a string literal. Phase 3 introduced `ext::PROFILE_WIDGET_BUILDS` exactly to avoid this pattern. The call site is correct (the string matches), but it bypasses the constant — a future rename would not be caught by grep.
- **L2.** `fetch_timeline_chunk` at lines 221–223 uses `since_micros as i64` and `extent_micros as i64` without a ceiling guard. The doc comment acknowledges the truncation is theoretically unsafe ("Real timeline values stay well under `i64::MAX`"). A pathological VM Service response could push the watermark to a wrapping value.
- **L6.** Three of five tests for `set_profile_widget_builds` in `crates/fdemon-daemon/src/vm_service/extensions/performance.rs:81–125` test only `Option<bool>.map(|e| e.to_string())` — stdlib operations that don't need testing. Only `set_profile_widget_builds_round_trips_enabled_true` and `set_profile_widget_builds_uses_correct_extension_name` add real value.

## Acceptance Criteria

1. **M5 resolved.** `enable_frame_tracking` calls `handle.call_extension(crate::vm_service::extensions::ext::PROFILE_WIDGET_BUILDS, ...)` using the constant. Add the import as needed (`use super::extensions::ext;` or fully-qualified path).
   - Grep verification: `rg '"ext\.flutter\.profileWidgetBuilds"' crates/fdemon-daemon/` returns ONLY the constant definition in `extensions/mod.rs`. No other string-literal call sites remain.
2. **L2 resolved.** In `fetch_timeline_chunk`:
   - Add a ceiling guard before each `u64 → i64` cast: `since_micros.min(i64::MAX as u64) as i64` and `extent_micros.min(i64::MAX as u64) as i64`.
   - Remove the "in practice safe" caveat from the doc comment, replacing it with an explicit invariant: "Values are clamped to `i64::MAX` before the cast — sub-`i64::MAX` inputs round-trip cleanly; pathological values are silently clamped to the maximum, which the VM Service will reject as a window-too-large error and the polling loop will recover on the next tick."
3. **L6 resolved.** Either:
   - **Option (a):** Replace the three `Option<bool>.map(|e| e.to_string())` tests with real round-trip tests that drive `set_profile_widget_builds` end-to-end through a mock RPC and assert the encoded request shape.
   - **Option (b):** Delete the three stdlib-only tests and add a doc comment on the test module noting that round-trip coverage is provided by `toggle_bool_extension`'s tests in the parent module.
   - Either choice is acceptable. (a) is more thorough; (b) is faster and reduces test maintenance.
4. `cargo fmt --all -- --check && cargo check -p fdemon-daemon && cargo test -p fdemon-daemon && cargo clippy -p fdemon-daemon --all-targets -- -D warnings` all pass.

## Files Modified (Write)

- `crates/fdemon-daemon/src/vm_service/timeline.rs` — M5 (constant migration in `enable_frame_tracking`) + L2 (ceiling guards in `fetch_timeline_chunk`).
- `crates/fdemon-daemon/src/vm_service/extensions/performance.rs` — L6 (test cleanup, option a or b).

## Files Read (Dependencies)

- `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` — read-only: confirm `ext::PROFILE_WIDGET_BUILDS` is `pub(crate)` and visible from `vm_service/timeline.rs`.

## Approach Hints

- For M5: the import path depends on module visibility. If `ext` is `pub(crate)`, `use crate::vm_service::extensions::ext;` from `timeline.rs` should work. Verify by inspection.
- For L2: a constant `const I64_MAX_AS_U64: u64 = i64::MAX as u64;` near the top of the function (or as a module constant) makes the intent clearer than the inline expression. Optional but recommended.
- For L6: lean toward option (b) (delete + note) unless adding real round-trip tests would also catch other regressions. The existing `set_profile_widget_builds_round_trips_enabled_true` is sufficient coverage.

## Out of Scope

- Other uses of string literals for extension names in `fdemon-daemon` outside `vm_service/timeline.rs:147` — those, if any, are separate cleanups.
- Changing `fetch_timeline_chunk`'s signature or behavior beyond the cast guard.
- Adding new tests for `enable_frame_tracking` — its tests live elsewhere and are unchanged.
- Restructuring `extensions/performance.rs` module layout.
