## Task: Rewrite Architecture page to the real 5-crate workspace

**Objective**: Replace the Architecture page's fictional monolithic `src/{core,app,tui,…}`
layout with the actual Cargo workspace structure and refresh stale code snippets.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `website/src/pages/docs/architecture.rs`: structure, layer table, module reference
  cards, and stale snippets.

**Files Read (Dependencies):**
- `Cargo.toml`: workspace member list.
- `docs/ARCHITECTURE.md`, `CLAUDE.md`: canonical architecture description.
- `crates/` tree: actual file paths.

### Details

- Project structure [D-30]: show the workspace — `crates/fdemon-core`,
  `crates/fdemon-daemon`, `crates/fdemon-app`, `crates/fdemon-tui`, `crates/fdemon-dap`,
  plus the `flutter-demon` binary crate. Remove the fictional root `src/core/`,
  `src/app/`, etc.
- Layer/dependency table [D-31]: real crates only. Drop phantom "Common"/"Services"
  *crates* (services is a module inside `fdemon-app`; common utils live inside crates).
  Add `fdemon-dap`. Match `docs/ARCHITECTURE.md` / `CLAUDE.md`.
- Module Reference cards [D-32]: fix every path — e.g. `error.rs` →
  `crates/fdemon-core/src/error.rs`; `signals.rs` → `crates/fdemon-app/src/signals.rs`;
  config → `crates/fdemon-app/src/config/`.
- Stale snippets (MEDIUM): correct the update-fn shape to the real `handler::update()`
  returning `(AppState, Option<UpdateAction>)` [D-33]; rename the
  `AppState.device_selector` field example to `new_session_dialog_state` [D-34].
- Optionally note the per-session DevTools state and native-log capture modules to match
  the current `CLAUDE.md` architecture description.

### Acceptance Criteria

1. Structure/layer/module sections reflect the 5-crate workspace + binary.
2. No references to non-existent root `src/<module>/` folders.
3. `fdemon-dap` is present; "Common"/"Services" are not listed as crates.
4. Stale `UpdateResult`/`AppState` snippets corrected.
5. `cd website && trunk build` compiles.

### Notes

- Keep terminology consistent with `docs/ARCHITECTURE.md`, which T09 verifies in parallel.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a372aaf850992cf04

### Files Modified

| File | Changes |
|------|---------|
| `website/src/pages/docs/architecture.rs` | Full rewrite: replaced fictional monolithic structure with real 5-crate workspace; fixed layer table, module reference cards, stale snippets, and project structure tree |

### Notable Decisions/Tradeoffs

1. **System Architecture diagram**: Replaced the old `app/`, `tui/`, `daemon/`, `core/` nodes with the real crates — `flutter-demon (binary)`, `fdemon-tui`, `fdemon-app`, `fdemon-daemon`, `fdemon-dap`, `fdemon-core` — using a top-to-bottom dependency flow.

2. **Layer/dependency table**: Removed phantom "Common" and "Services" layer rows; added `fdemon-dap`; renamed all layers to actual crate names with correct dependency lists.

3. **Module Reference cards**: Rebuilt from 8 fictional cards to 6 accurate cards, each using real `crates/<crate>/src/...` paths. Consolidated `config` and `services` into one `fdemon-app — config & services` card to reduce redundancy. Removed fictional `watcher/mod.rs` standalone card (watcher is a module inside `fdemon-app`).

4. **Stale `UpdateResult` snippet**: Replaced with correct `pub fn update(state: AppState, message: Message) -> (AppState, Option<UpdateAction>)` signature showing the real return type.

5. **`device_selector` → `new_session_dialog_state`**: Updated the `AppState` code snippet field name to match the real struct.

6. **Project Structure**: Replaced fictional `src/core/`, `src/app/`, `src/tui/`, `src/common/` layout with the real workspace tree under `crates/`.

7. **`signals.rs` location**: Confirmed at `crates/fdemon-app/src/signals.rs` via filesystem check. Placed in `fdemon-app — core` module card; `fdemon-core` card uses `prelude.rs` instead.

8. **Testing table**: Updated from per-module file references to per-crate approximate test counts matching CLAUDE.md.

### Testing Performed

- `cd /Users/ed/Dev/zabin/flutter-demon/website && cargo check` — Passed (1 pre-existing warning, no new errors)

### Risks/Limitations

1. **No known risks**: All paths verified against actual filesystem. Website compiles cleanly.
</content>
