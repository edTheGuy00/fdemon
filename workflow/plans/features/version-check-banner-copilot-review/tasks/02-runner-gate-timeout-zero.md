# Task 02 — Treat `version_check_timeout_secs = 0` as fully disabled

**Plan**: [../PLAN.md](../PLAN.md)
**Agent**: `implementor`
**Resolves**: Copilot review comments **3** and **4** on PR #49.

---

## Objective

When the user sets `[behavior] version_check_timeout_secs = 0`, do not
call `spawn_version_check` at all — match the documented "equivalent to
disabling the check" semantics with zero outbound network activity.

---

## Background

Both `crates/fdemon-tui/src/runner.rs:78-85` (run_with_project) and
`:208-215` (run_with_project_and_dap) gate the spawn only on the
`version_check` bool, then pass
`Duration::from_secs(version_check_timeout_secs as u64)` to
`spawn_version_check`. With `timeout_secs = 0`, reqwest still builds the
client and initiates the request before timing out — so DNS / TLS / a
brief outbound HTTP attempt do happen, contradicting these docs:

- `crates/fdemon-app/src/config/types.rs:171-173` —
  "A value of 0 is equivalent to disabling the check."
- `docs/CONFIGURATION.md:264` —
  "A value of `0` is equivalent to disabling the check. Has no effect when `version_check = false`."
- `docs/CONFIGURATION.md:320` —
  "A value of `0` disables the check (equivalent to setting `version_check = false`)."

---

## Files

- `crates/fdemon-tui/src/runner.rs` — both call sites get an extended gate.
- *Optional but recommended*: `crates/fdemon-app/src/config/types.rs` — add a `pub(crate)` helper `BehaviorSettings::should_run_version_check(&self) -> bool` so both runner sites share the predicate and the rule is unit-testable in one place.

---

## Implementation

### Option A — inline gate (smallest diff)

In both `crates/fdemon-tui/src/runner.rs:78-85` and `:208-215`, change:

```rust
if engine.settings.behavior.version_check {
    spawn::spawn_version_check(
        engine.msg_sender(),
        std::time::Duration::from_secs(
            engine.settings.behavior.version_check_timeout_secs as u64,
        ),
    );
}
```

to:

```rust
if engine.settings.behavior.version_check
    && engine.settings.behavior.version_check_timeout_secs > 0
{
    spawn::spawn_version_check(
        engine.msg_sender(),
        std::time::Duration::from_secs(
            engine.settings.behavior.version_check_timeout_secs as u64,
        ),
    );
}
```

### Option B — shared helper (preferred — testable, no duplication)

1. In `crates/fdemon-app/src/config/types.rs`, alongside `BehaviorSettings`:

   ```rust
   impl BehaviorSettings {
       /// Returns `true` when the startup version check should run.
       ///
       /// Both the explicit `version_check` bool and a non-zero
       /// `version_check_timeout_secs` are required. A zero timeout is
       /// documented as equivalent to disabling the check, so we honor
       /// that at the call site (no outbound HTTP attempt at all).
       pub(crate) fn should_run_version_check(&self) -> bool {
           self.version_check && self.version_check_timeout_secs > 0
       }
   }
   ```

2. In `crates/fdemon-tui/src/runner.rs`, replace both inline conditions with:

   ```rust
   if engine.settings.behavior.should_run_version_check() {
       spawn::spawn_version_check(
           engine.msg_sender(),
           std::time::Duration::from_secs(
               engine.settings.behavior.version_check_timeout_secs as u64,
           ),
       );
   }
   ```

3. Add unit tests next to the existing `BehaviorSettings` tests:

   ```rust
   #[test]
   fn should_run_version_check_when_enabled_with_positive_timeout() {
       let s = BehaviorSettings {
           version_check: true,
           version_check_timeout_secs: 3,
           ..Default::default()
       };
       assert!(s.should_run_version_check());
   }

   #[test]
   fn should_not_run_version_check_when_disabled() {
       let s = BehaviorSettings {
           version_check: false,
           version_check_timeout_secs: 3,
           ..Default::default()
       };
       assert!(!s.should_run_version_check());
   }

   #[test]
   fn should_not_run_version_check_when_timeout_is_zero() {
       let s = BehaviorSettings {
           version_check: true,
           version_check_timeout_secs: 0,
           ..Default::default()
       };
       assert!(!s.should_run_version_check());
   }
   ```

   `should_run_version_check` is `pub(crate)`, so the test must live in
   the same crate (`fdemon-app`). The existing test module in
   `config/types.rs` is fine.

**Visibility note**: `runner.rs` lives in `fdemon-tui`, which depends on
`fdemon-app`. The helper must be `pub` (not `pub(crate)`) for the runner
to call it from a different crate. Change `pub(crate)` to `pub` if
choosing Option B.

---

## Acceptance criteria

- [ ] `version_check_timeout_secs = 0` causes both TUI entry points (`run_with_project` and `run_with_project_and_dap`) to skip the `spawn_version_check` call entirely.
- [ ] If Option B is chosen: `should_run_version_check` returns `true` only when both `version_check == true` and `version_check_timeout_secs > 0`; covered by three unit tests.
- [ ] No documentation change required — the existing docs already describe the implemented behavior after this fix.
- [ ] `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` all green.

---

## Notes

- Do NOT remove the `version_check: bool` field — it remains the primary opt-out and is independently useful (preserves a non-zero default timeout for users who toggle the check back on).
- Do NOT add a runtime warning or toast when `timeout_secs = 0` — the user explicitly chose that value, and the docs explicitly call it out as a way to disable.
- Recommend **Option B**. The shared helper makes the intent explicit, removes duplication, and gives a place to hang a regression test. Option A is a fine fallback if the implementor wants to keep the diff micro.
- Do NOT add an integration test that asserts no network call happens. A unit test on the predicate is sufficient; the integration path is exercised by the existing wiremock suite which already covers the spawn-and-respond loop.

---

## Completion Summary

**Status:** Done
**Branch:** feat/version-check-banner

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/config/types.rs` | Added `impl BehaviorSettings` with `pub fn should_run_version_check(&self) -> bool` helper; added three unit tests |
| `crates/fdemon-tui/src/runner.rs` | Updated both `run_with_project` and `run_with_project_and_dap` to use `should_run_version_check()` instead of the bare `version_check` bool |

### Notable Decisions/Tradeoffs

1. **Option B chosen**: Implemented the shared `should_run_version_check()` helper on `BehaviorSettings` rather than Option A (inline gate). The visibility is `pub` (not `pub(crate)`) so the `fdemon-tui` crate can call it across the crate boundary, as noted in the task's visibility note.
2. **No doc change needed**: Existing docs in `config/types.rs` and `docs/CONFIGURATION.md` already correctly describe the `timeout_secs = 0` semantics; the fix makes the code match the docs.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no warnings)
- `cargo test --workspace` - Passed (all 3 new tests + full suite: zero failures across all crates)

### Risks/Limitations

1. **None**: The change is a pure gate addition. When `version_check_timeout_secs > 0` and `version_check = true` (the defaults), behavior is identical to before. Only the `timeout_secs = 0` edge case is now handled correctly.
