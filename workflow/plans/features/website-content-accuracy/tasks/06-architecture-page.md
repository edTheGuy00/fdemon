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
</content>
