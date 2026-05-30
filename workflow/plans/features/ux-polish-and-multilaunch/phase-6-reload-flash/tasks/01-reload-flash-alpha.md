## Task: Reload-flash alpha helper (`Session::reload_flash_alpha`)

**Objective**: Add a pure helper on `Session` that turns the existing
`last_reload_time` timestamp into a `0.0→1.0` flash intensity that decays over
~500 ms, suppressed outside a steady `Running` phase. No new state, no timer; the
TUI (task 02) reads this to tint the header. This task adds **no consumer** — it
must build and test on its own.

**Depends on**: None

**Estimated Time**: 0.5–1h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/session/session.rs`: add `reload_flash_alpha` + a
  named duration constant, alongside the existing reload helpers
  (`complete_reload`, `last_reload_display`), plus inline `#[cfg(test)]` tests.

**Files Read (Dependencies):**
- `crates/fdemon-core/src/types.rs`: `AppPhase` (for the `Running` guard).

### Details

`complete_reload()` (`session.rs:617`) already does the heavy lifting — it stamps
`self.last_reload_time = Some(Local::now())` and sets `phase = AppPhase::Running`.
The new helper only reads that timestamp:

```rust
/// Duration of the post-reload success flash. The header tint fades from full
/// to none over this window.
const RELOAD_FLASH_DURATION_MS: i64 = 500;

/// Intensity of the reload-success flash at wall-clock `now`, in `[0.0, 1.0]`.
///
/// Returns `1.0` at the instant of `complete_reload()` and decays linearly to
/// `0.0` over `RELOAD_FLASH_DURATION_MS`, staying `0.0` afterwards. Returns
/// `0.0` when the session never reloaded or is not in a steady `Running` phase
/// (so the flash never bleeds into `Stopped`/`Quitting`/error states — a failed
/// reload leaves the phase at `Running` and does not stamp `last_reload_time`,
/// so only successful reloads can trigger it).
///
/// `now` is injected (rather than read internally) to keep the helper pure and
/// unit-testable; the render path passes `Local::now()`.
pub fn reload_flash_alpha(&self, now: DateTime<Local>) -> f32 {
    if self.phase != AppPhase::Running {
        return 0.0;
    }
    let Some(reloaded_at) = self.last_reload_time else {
        return 0.0;
    };
    let elapsed_ms = (now - reloaded_at).num_milliseconds();
    if !(0..RELOAD_FLASH_DURATION_MS).contains(&elapsed_ms) {
        return 0.0; // future timestamp, or window elapsed
    }
    1.0 - (elapsed_ms as f32 / RELOAD_FLASH_DURATION_MS as f32)
}
```

Notes for the implementor:
- `DateTime`/`Local` are already imported in this file (used by
  `session_duration`, `last_reload_display`). Reuse the existing imports.
- Guard against a negative `elapsed_ms` (clock skew / `now` before the stamp) —
  the `0..DURATION` range check above handles both ends; do not let it produce a
  value `> 1.0` or `< 0.0`.
- Keep the function under 50 lines and give it a `///` doc comment
  (CODE_STANDARDS). `RELOAD_FLASH_DURATION_MS` is a named constant with a
  derivation comment — no magic `500` inline.
- This is the only place the `500 ms` lives; task 02 must not re-introduce a
  duration constant.

### Acceptance Criteria

1. `reload_flash_alpha(now)` returns `1.0` when `now == last_reload_time` and the
   phase is `Running`.
2. It decays linearly: at `now = last_reload_time + 250 ms` it returns ~`0.5`
   (within a small epsilon).
3. It returns `0.0` at/after `last_reload_time + RELOAD_FLASH_DURATION_MS`.
4. It returns `0.0` when `last_reload_time` is `None` (never reloaded).
5. It returns `0.0` when the phase is not `Running` (e.g. `Stopped`,
   `Reloading`, `Quitting`) even if `last_reload_time` is recent.
6. It returns `0.0` (never negative, never `> 1.0`) for a `now` earlier than
   `last_reload_time` (clock skew).
7. The result is always within `[0.0, 1.0]`.

### Testing

Construct fixed timestamps (`Local::now()` captured once, then offset with
`chrono::Duration::milliseconds(..)`) so the tests are deterministic. Build the
`Session` via the existing test constructor and set `phase` / `last_reload_time`
directly (the test module is in-crate, so private fields are reachable; mirror how
neighboring session tests set up state).

```rust
#[test]
fn flash_alpha_full_at_reload_instant() { /* now == last_reload_time → 1.0 */ }

#[test]
fn flash_alpha_half_at_midpoint() { /* +250ms → ~0.5 */ }

#[test]
fn flash_alpha_zero_after_window() { /* +500ms and +1s → 0.0 */ }

#[test]
fn flash_alpha_zero_when_never_reloaded() { /* last_reload_time = None → 0.0 */ }

#[test]
fn flash_alpha_suppressed_when_not_running() { /* phase = Stopped, recent reload → 0.0 */ }

#[test]
fn flash_alpha_zero_for_past_now() { /* now < last_reload_time → 0.0, not negative */ }
```

### Notes

- **No new field.** The plan's optional `reload_flash_alpha()` is realized as a
  method over existing state; do not add a field to `Session` or `AppState`.
- **No caller in this task.** The TUI integration is task 02. If clippy flags the
  new `pub fn` as unused within `fdemon-app`, that is expected — do **not** add
  `#[allow(dead_code)]` (a `pub` method on a `pub` struct is part of the crate's
  API surface and is not dead-code-linted); task 02 will consume it.
- **Why `phase == Running` and not `is_running()`:** `complete_reload()` sets the
  phase to exactly `Running`, so the steady-state check is a direct equality.
  Using `Running` (not `Reloading`) also means an in-flight *new* reload does not
  show a stale flash from the previous one.
