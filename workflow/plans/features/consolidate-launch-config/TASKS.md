# Task Index — Consolidate Launch Config (Option B)

Plan: [PLAN.md](./PLAN.md)

**Approved direction (2026-04-24):** Option B — invert priority so `launch.toml auto_start = true` beats cached `settings.local.toml`, add symmetric persistence so manual dialog launches also update the cache, and drop the redundant `[behavior] auto_start` global flag.

## Tasks

| # | Task | File | Agent | Status | Depends on |
|---|------|------|-------|--------|------------|
| 01 | Invert `find_auto_launch_target` priority — launch.toml `auto_start = true` wins over `settings.local.toml` cache | [tasks/01-invert-auto-launch-priority.md](./tasks/01-invert-auto-launch-priority.md) | implementor | [x] Done | — |
| 02 | Symmetric persistence — save `last_device` / `last_config` on manual NewSessionDialog launches too | [tasks/02-symmetric-persistence.md](./tasks/02-symmetric-persistence.md) | implementor | [x] Done | — |
| 03 | Remove `[behavior] auto_start` global flag (redundant with per-config `auto_start`) | [tasks/03-remove-global-auto-start.md](./tasks/03-remove-global-auto-start.md) | implementor | [x] Done | — |
| 04 | Documentation — rewrite CONFIGURATION.md priority section, add TESTING.md regression test J | [tasks/04-docs-rewrite.md](./tasks/04-docs-rewrite.md) | implementor | [x] Done (CONCERN resolved: follow-up fixed gate/fall-through doc inaccuracies) | 01, 02, 03 (logically — merges last to describe final state) |

## Wave Plan

- **Wave 1:** Tasks 01, 02, 03 in parallel (disjoint write files — see overlap matrix).
- **Wave 2:** Task 04 after all three of Wave 1 merge.

Task 04 is not a *code* dependency on 01/02/03 — the docs would still compile if they merged simultaneously — but describing the final state cleanly requires the code to be settled first. Run 04 sequentially in the same branch after the merge.

## File Overlap Analysis

### Files Modified (Write) — per task

| Task | Files Modified (Write) | Files Read (dependency) |
|------|------------------------|--------------------------|
| 01 | `crates/fdemon-app/src/spawn.rs` | `crates/fdemon-app/src/config/settings.rs` (LastSelection types), `crates/fdemon-app/src/config/priority.rs` (`get_first_auto_start`) |
| 02 | `crates/fdemon-app/src/handler/new_session/launch_context.rs` | `crates/fdemon-app/src/config/settings.rs` (reads `save_last_selection` signature) |
| 03 | `crates/fdemon-app/src/config/types.rs`, `crates/fdemon-tui/src/startup.rs`, `crates/fdemon-app/src/config/settings.rs`, `crates/fdemon-app/src/settings_items.rs`, `example/app1/.fdemon/config.toml`, `example/app2/.fdemon/config.toml`, `example/app3/.fdemon/config.toml`, `example/app4/.fdemon/config.toml`, `example/app5/.fdemon/config.toml` | — |
| 04 | `docs/CONFIGURATION.md`, `example/TESTING.md` | all of Wave 1 read-only |

### Overlap Matrix (Wave 1 peers)

|        | 01 | 02 | 03 |
|--------|----|----|----|
| **01** | —  | none (different files) | none (reads `settings.rs`, Task 03 writes `settings.rs` — see note below) |
| **02** | none | — | none (reads `settings.rs`, Task 03 writes `settings.rs` — see note below) |
| **03** | read-only on `spawn.rs` | read-only on `launch_context.rs` | — |

**Note on `settings.rs`:** Task 03 writes `settings.rs` (to remove any `BehaviorSettings::auto_start` serialization paths if present — verify in the task). Tasks 01 and 02 only *read* `settings.rs` for the `save_last_selection` / `LastSelection` signatures. Task 03 does NOT change those signatures. So as long as Task 03 preserves the `save_last_selection`/`load_last_selection` API surface, parallel execution is safe.

If Task 03 discovers it needs to change `save_last_selection`'s signature, stop and coordinate — escalate back to the planner.

**Strategy:** All three Wave-1 tasks run in parallel with worktree isolation. Zero shared write files. Task 04 follows sequentially in the merged branch.

## Documentation Updates

### Managed by doc_maintainer
**None.** The changes do not:
- Add, remove, or rename modules (BehaviorSettings stays; one field is removed).
- Change layer dependencies or crate boundaries.
- Introduce new coding patterns.
- Add new build commands.

So **no updates to `docs/ARCHITECTURE.md`, `docs/CODE_STANDARDS.md`, or `docs/DEVELOPMENT.md`** are needed.

### Unmanaged docs (Task 04)
- `docs/CONFIGURATION.md` — rewrite the launch-config priority section; remove `[behavior] auto_start` reference and replace with a deprecation note; correct the documented priority chain.
- `example/TESTING.md` — add Test J (launch.toml edit takes effect even with stale `settings.local.toml`).

## Verification (run once after all four tasks merge)

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Manual smoke test (the scenario that surfaced this plan):

1. With 4 devices connected (Android + iOS + macOS + Chrome), delete `example/app3/.fdemon/settings.local.toml` for a clean start.
2. Set `example/app3/.fdemon/launch.toml` Development config to `device = "android"` + `auto_start = true`.
3. Run `cargo run -- example/app3` → confirm session starts on Android.
4. Quit. Verify `settings.local.toml` now contains `last_device = "bd4775f2"` and `last_config = "Development"`.
5. Edit `launch.toml` → change `device = "macos"` (keep `auto_start = true`).
6. Run `cargo run -- example/app3` again. **Expected: session starts on macOS** (launch.toml auto_start wins over cached Android selection). Today this is broken — session starts on Android.
7. Remove `auto_start = true` from the Development config. Keep `settings.local.toml` with `last_device = "macos"`.
8. Run again. Expected: session starts on macOS from cache (Priority 2 falls through).
9. Use the manual NewSessionDialog to select iPhone Air. Quit. Confirm `settings.local.toml` now shows `last_device = "B8D70379-..."` (Task 02 fix — today it wouldn't update).

## Out of scope (explicit non-goals)

- Changing `LaunchConfig.device` from `String` to an enum.
- Redesigning the NewSessionDialog UX.
- Collapsing `config.toml` + `launch.toml` + `settings.local.toml` into a single file (Option C was rejected — can't gitignore a section of a tracked file).
- Adding a "Clear last selection" button to the Settings Panel (flagged as optional polish in PLAN.md §8; defer to a follow-up).
- Migration tooling for users who had `[behavior] auto_start = true` and relied on it — serde silently ignores the removed field, which is the graceful path; no explicit migration needed.

## Risks

1. **`[behavior] auto_start` removal is a breaking config change.** Mitigated: serde ignores unknown fields by default (verified — no `deny_unknown_fields` on `Settings` or `BehaviorSettings`), so existing configs still load. Users who set the flag will see it silently stop doing anything; Task 03 includes a one-time `warn!` on load when the field is present.
2. **Task 01's priority inversion changes user-observable behavior for users who relied on cache overriding launch.toml.** This is exactly the bug being fixed, so it's intentional — but worth calling out in CHANGELOG (Task 04).
3. **Task 02 may surface an unrelated timing issue** if `save_last_selection` writes to disk synchronously on a hot path. Verify in Task 02 that the persistence call doesn't block session spawn.
