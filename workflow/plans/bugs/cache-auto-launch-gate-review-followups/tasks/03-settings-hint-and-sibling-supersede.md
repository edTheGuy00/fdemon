# Task 03 — Settings Panel "restart required" hint + sibling-bug supersede note

**Plan:** [../BUG.md](../BUG.md) · **Index:** [../TASKS.md](../TASKS.md)
**Agent:** implementor
**Depends on:** —
**Wave:** 1 (parallel with Task 01)

## Goal

Resolve review findings **M1** (Settings Panel toggle gives no "takes effect on next launch" hint) and the *sibling-`TASKS.md`-half* of **C4** (sibling-bug coordination undocumented in plan files). The header-comment-half of C4 is owned by Task 01.

Two trivial edits, packaged together for orchestration efficiency.

## Files Modified (Write)

| File | Change |
|------|--------|
| `crates/fdemon-app/src/settings_items.rs` | Update the `behavior.auto_launch` row's `.description(...)` string. Currently: `"Auto-launch the last-used device on startup (skipped if launch.toml has auto_start)"`. Replace with: `"Auto-launch the last-used device on startup (takes effect on next fdemon launch; skipped if launch.toml has auto_start)"`. Single line edit at line ~92. |
| `workflow/plans/bugs/launch-toml-device-ignored/TASKS.md` | Mark the Task 03 row as SUPERSEDED with a back-reference. Replace the existing Task 03 row's status text (or add a new column / inline annotation) with: `SUPERSEDED — wiring absorbed by cache-auto-launch-gate Task 04 on 2026-04-29; close as resolved-by-absorption when reviewed`. The exact placement depends on the sibling TASKS.md format; inspect the file and pick the cleanest spot (table cell, status line, or trailing note section). |

## Files Read (dependency)

- `workflow/plans/bugs/launch-toml-device-ignored/TASKS.md` (read before editing to understand its structure)

## Implementation Notes

### Settings Panel description

Locate `crates/fdemon-app/src/settings_items.rs` line ~92 and edit the `.description(...)` argument. The change is a string-literal replacement; no code logic changes.

Before:
```rust
SettingItem::new("behavior.auto_launch", "Auto-launch on cached device")
    .description("Auto-launch the last-used device on startup (skipped if launch.toml has auto_start)")
    .value(SettingValue::Bool(settings.behavior.auto_launch))
    .default(SettingValue::Bool(false))
    .section("Behavior"),
```

After:
```rust
SettingItem::new("behavior.auto_launch", "Auto-launch on cached device")
    .description("Auto-launch the last-used device on startup (takes effect on next fdemon launch; skipped if launch.toml has auto_start)")
    .value(SettingValue::Bool(settings.behavior.auto_launch))
    .default(SettingValue::Bool(false))
    .section("Behavior"),
```

### Sibling-bug supersede note

Read `workflow/plans/bugs/launch-toml-device-ignored/TASKS.md` first to understand its current structure. The Task 03 row is in a markdown table at the top of the file. Most-likely format:

```markdown
| 03 | Wire `launch.toml` into headless auto-start ... | [tasks/03-...](./tasks/03-...) | implementor | — |
```

Add a SUPERSEDED prefix to the task description (or append a status note). Two acceptable formats:

**Option A — inline prefix (preferred for visibility):**
```markdown
| 03 | ⚠️ SUPERSEDED 2026-04-29 — wiring absorbed by [`cache-auto-launch-gate` Task 04](../cache-auto-launch-gate/tasks/04-headless-gate.md). Close as resolved-by-absorption when reviewed. | [tasks/03-headless-launch-toml-auto-launch.md](./tasks/03-headless-launch-toml-auto-launch.md) | implementor | — |
```

**Option B — separate note section at the bottom:**
```markdown
---

## Status Updates

- **2026-04-29 — Task 03 SUPERSEDED:** The headless `find_auto_launch_target` wiring scoped to this task was absorbed inline by [`cache-auto-launch-gate` Task 04](../cache-auto-launch-gate/tasks/04-headless-gate.md). Close Task 03 as resolved-by-absorption when reviewed. See `src/headless/runner.rs` near `headless_auto_start` for the inline header comment that mirrors this note.
```

Implementor: pick whichever fits the file's existing style. Option A is more discoverable; Option B is less intrusive. If the file has a "Status Updates" or similar section already, use Option B. Otherwise use Option A.

### No code logic changes

Both edits are documentation/string changes. No tests need updating. `cargo test --workspace` should pass without modification.

## Verification

- `cargo check --workspace --all-targets` (sanity — string edit shouldn't break compilation)
- `cargo test -p fdemon-app settings_items` (the existing `test_behavior_auto_launch_item_present` test should still pass; it checks for `id == "behavior.auto_launch"`, not the description text)
- Manual: open `fdemon`, hit `S`, navigate to Behavior tab, verify the description for `Auto-launch on cached device` reads with the new "takes effect on next fdemon launch" clause.
- Manual: `cat workflow/plans/bugs/launch-toml-device-ignored/TASKS.md | grep -A 1 "03"` shows the SUPERSEDED note.

## Acceptance

- [ ] `crates/fdemon-app/src/settings_items.rs` description for `behavior.auto_launch` includes the substring `takes effect on next fdemon launch`.
- [ ] `workflow/plans/bugs/launch-toml-device-ignored/TASKS.md` Task 03 entry is annotated SUPERSEDED with a date (2026-04-29) and a back-reference link/path to `cache-auto-launch-gate` Task 04.
- [ ] Existing `test_behavior_auto_launch_item_present` still passes.
- [ ] `cargo clippy --workspace -- -D warnings` clean.
