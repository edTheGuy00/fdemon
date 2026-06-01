# Bug Fix Plan: PR #50 Copilot Review Findings

**PR:** #50 — feat: UX polish & multi-device launch
**Branch:** `feat/ux-polish-and-multilaunch`
**Source:** 3 Copilot inline review comments

## Verdict Summary

| # | Location | Verdict | Severity |
|---|----------|---------|----------|
| 1 | `handler/new_session/launch_context.rs:646` (pre-app SDK gate) | **Valid — fix** | Medium |
| 2 | `config/launch.rs:348` (extra_args `-` filter) | **Valid — fix** | Medium |
| 3 | `new_session_dialog/target_selector_state.rs:249` (stale count) | **Valid — fix (minor)** | Low |

All three reproduce against the current code. Details and fixes below.

---

## Bug 1 — Pre-app launch leaves an orphaned `Preparing` session when no SDK

### Confirmation
`spawn_one()` resolves `flutter_opt = state.flutter_executable()` at line 646, but only the
**direct** branch (`else`, lines 734–745) checks it and rolls back the session via
`remove_session` on `None`. The **`SpawnPreAppSources`** branch (lines 718–732) creates the
session, sets phase to `Preparing`, and returns the action **without ever checking the SDK**.

Later, `Message::PreAppSourcesReady` (`handler/update.rs:2888–2895`) re-resolves the SDK and,
on `None`, logs a warning and returns `UpdateResult::none()` — **without removing the session**.

**Result:** when native logs with pre-app sources are enabled and no Flutter SDK is resolved,
the session is created, stuck permanently in `Preparing`, and consumes one of the 9 session
slots. No error is surfaced to the user.

### Fix
Make the pre-app path fail-fast like the direct path. In `spawn_one`, before creating the
session (or before returning the `SpawnPreAppSources` action), guard on `flutter_opt.is_none()`
and return `Err(<same "No Flutter SDK found…" message>)` for the pre-app branch too. Because
`flutter_executable()` is state-global, a missing SDK now will still be missing at
`PreAppSourcesReady`, so failing early loses nothing and is consistent.

- Cleanest: resolve the SDK guard once near the top of `spawn_one` (after the dedup check) and
  return `Err` for **both** branches, eliminating the divergence entirely. The `else` branch
  then no longer needs its own `flutter_opt` unwrap/rollback.
- The fan-out loop already routes `Err(reason)` into `skipped`, so the missing-SDK case will
  surface the standard "Device skipped: No Flutter SDK found…" toast/error — same UX as today's
  direct path.

### Defense-in-depth (optional)
In `PreAppSourcesReady`'s `None` arm, also call `remove_session(session_id)` and push a Warn
toast, so any future code path that reaches `Preparing` without an SDK still self-heals instead
of leaking a slot.

### Tests
- `spawn_one` with pre-app sources enabled + no resolved SDK → returns `Err`, no session left in
  `session_manager`.
- `handle_launch` (multi-device) with pre-app sources + no SDK → all devices skipped, error
  surfaced, zero sessions created.

### Files
- `crates/fdemon-app/src/handler/new_session/launch_context.rs` (write)
- `crates/fdemon-app/src/handler/update.rs` (write — optional defense-in-depth)

---

## Bug 2 — `extra_args` value tokens silently dropped (`--web-port 8080` → `--web-port`)

### Confirmation
`build_flutter_args` (launch.rs:347–354) keeps an `extra_args` entry only if
`arg.starts_with('-')`. But `parse_tool_args` (`config/vscode.rs:237–239`) pushes any
unrecognized token verbatim into `extra_args`, including **value tokens** of split flag/value
pairs. A VS Code `toolArgs` of `["--web-port", "8080"]` becomes
`extra_args = ["--web-port", "8080"]`; the `8080` is then dropped, producing the malformed
invocation `flutter run … --web-port` (flag with no value).

### Fix
Relax the validation so a value token following a flag is preserved. Recommended approach:
treat the `extra_args` list positionally rather than per-token. Walk the list; when an entry
starts with `-` and does **not** contain `=`, accept the **next** entry as its value
(regardless of leading `-`). Keep the existing NUL-byte and `MAX_EXTRA_ARG_LEN` checks on
every retained token. A leading bare value (no preceding flag) is still dropped, preserving the
original intent of rejecting stray positionals.

- Simpler alternative if positional tracking is deemed too clever: drop the `starts_with('-')`
  check entirely and keep only the NUL + length guards. The original justification for the
  `-` rule (the doc comment) explicitly notes there is **no shell-evaluation risk** since args
  reach `Command::args()` as separate elements — so the `-` rule guards little. The positional
  approach is preferred because it still filters obvious stray positionals.

### Tests
- `build_flutter_args` with `extra_args = ["--web-port", "8080"]` → both tokens present, in order.
- `--foo=bar` (single token) still preserved.
- NUL byte / over-length tokens still dropped.
- A leading bare positional (e.g. `["8080"]` with no preceding flag) → dropped (documented).

### Files
- `crates/fdemon-app/src/config/launch.rs` (write)

---

## Bug 3 — Stale "N selected" count when a checked device becomes unsupported

### Confirmation
`set_connected_devices` (target_selector_state.rs:242–249) prunes `checked_device_ids` only for
ids **no longer present**. An id that is still present but whose `is_supported` flipped to
`false` after a refresh is retained. `checked_devices()` (line 457–462) filters on
`d.is_supported`, so such ids cannot launch; `checked_count()` (line 444–446) returns the raw
set length and still counts them. The dialog footer
(`widgets/new_session_dialog/target_selector.rs:352` and `mod.rs:1027`) renders
`"({} selected)"` from `checked_count()`, so it can show e.g. "2 selected" when only 1 device
is runnable.

This is acknowledged indirectly by the existing `checked_devices()` doc comment ("checked
before a refresh flipped `is_supported`") — the state is known-possible, just not pruned.

### Fix
Make pruning also drop ids whose device is now unsupported. In `set_connected_devices`, change
the `retain` predicate so an id is kept only if it maps to a **present and supported** device:

```
self.checked_device_ids
    .retain(|id| self.connected_devices.iter()
        .any(|d| d.id.as_str() == id.as_str() && d.is_supported));
```

(Or build a `supported_present` set first to avoid the borrow on `self` inside the closure.)
This makes `checked_count()` and `checked_devices()` agree without changing either accessor.
`checked_devices()` keeps its `is_supported` filter as a defensive safety net.

### Tests
- Seed two supported checked devices; refresh with one flipped to `is_supported = false` (same
  id present) → `checked_count() == 1` and matches `checked_devices().len()`.
- Existing presence-based pruning test still passes.

### Files
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` (write)

---

## File Overlap Analysis

| Task | Files Modified (Write) |
|------|------------------------|
| Bug 1 | `handler/new_session/launch_context.rs`, `handler/update.rs` |
| Bug 2 | `config/launch.rs` |
| Bug 3 | `new_session_dialog/target_selector_state.rs` |

No shared write files across the three tasks → all **Parallel (worktree)** safe. Each is small
and independent; they can also simply be done sequentially on the branch in one pass.

## Verification
- `cargo test -p fdemon-app`
- `cargo clippy --workspace`
- `cargo fmt --all`
