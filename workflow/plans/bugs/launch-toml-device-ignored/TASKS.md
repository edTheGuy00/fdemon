# Task Index — `launch.toml` device ignored on auto-launch

Plan: [BUG.md](./BUG.md)

## Tasks

| # | Task | File | Agent | Depends on |
|---|------|------|-------|------------|
| 01 | Alias `"macos"` ↔ `"darwin"` (and other display-string aliases) in `Device::matches` | [tasks/01-fix-platform-alias-matching.md](./tasks/01-fix-platform-alias-matching.md) | implementor | — |
| 02 | Surface "configured device not found" warning in the user-visible log buffer (keep `devices.first()` fallback) | [tasks/02-surface-device-miss-warning.md](./tasks/02-surface-device-miss-warning.md) | implementor | — |
| 03 | Wire `launch.toml` into headless auto-start (reuse `find_auto_launch_target`); fall back to `devices.first()` if no config | [tasks/03-headless-launch-toml-auto-launch.md](./tasks/03-headless-launch-toml-auto-launch.md) | implementor | — |

## Wave Plan

All three tasks are in **wave 1** — they have no inter-task dependencies and write to disjoint file sets.

## File Overlap Analysis

### Files Modified (Write)

| Task | Files Modified (Write) | Files Read (dependency) |
|------|------------------------|--------------------------|
| 01 | `crates/fdemon-daemon/src/devices.rs` | — |
| 02 | `crates/fdemon-app/src/spawn.rs` *(includes `pub` visibility change for `find_auto_launch_target` and `AutoLaunchSuccess` — both consumed by Task 03)* | `crates/fdemon-app/src/services/log.rs` (or current log-injection path) |
| 03 | `src/headless/runner.rs` | `crates/fdemon-app/src/spawn.rs`, `crates/fdemon-app/src/config/mod.rs`, `crates/fdemon-app/src/engine.rs`, `crates/fdemon-app/src/state.rs` |

Task 03 calls `find_auto_launch_target` and reads `AutoLaunchSuccess`'s shape from `spawn.rs` but **does not write to `spawn.rs`**. To make this safe in parallel:

- Task 02 owns all writes to `spawn.rs`, including the visibility change (`fn` → `pub fn`, `struct` → `pub struct`).
- Task 02 keeps the **signature** of `find_auto_launch_target` and the **shape** of `AutoLaunchSuccess` unchanged. The warning emission happens at the call site inside `spawn_auto_launch`, not by changing the function's interface.
- Task 03 builds against the assumption that `find_auto_launch_target` and `AutoLaunchSuccess` will be `pub` after Task 02 lands; if Task 03 finishes first, the orchestrator will need to merge Task 02 first to satisfy the visibility requirement.

### Overlap Matrix (wave-1 peers)

|        | 01 | 02 | 03 |
|--------|----|----|----|
| **01** | —  | none | none |
| **02** | none | — | read-only on `spawn.rs` from Task 03's side; Task 02 owns all writes |
| **03** | none | read-only on `spawn.rs` | — |

**Strategy:** all three tasks run in parallel with worktree isolation. Zero shared write files. Recommended merge order: Task 02 before Task 03 (to ensure visibility is in place when Task 03's call site compiles in `main`); Task 01 can merge in any order.

## Documentation Updates

None required:

- No modules added, removed, or renamed → ARCHITECTURE.md unchanged.
- No new patterns or layer crossings → CODE_STANDARDS.md unchanged.
- No new build/run/test steps → DEVELOPMENT.md unchanged.

The user-facing semantics of `launch.toml`'s `device` field are unchanged (specifier syntax is identical) — only the previously-broken cases now work — so user-facing CONFIGURATION docs (if any) are unaffected.

## Verification (run once after all three tasks merge)

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Manual smoke test:
1. With at least one iOS, Android, and macOS device connected, set `device = "macos"` (`auto_start = true`) in `.fdemon/launch.toml`.
2. Run `fdemon` from the project directory → confirm the macOS session is started.
3. Run `fdemon --headless` from the same project → confirm the macOS session is started in headless mode too.
4. Set `device = "ZZZ-not-a-real-id"` and re-run → confirm the user-visible warning appears in the log buffer and the fallback session still starts.
