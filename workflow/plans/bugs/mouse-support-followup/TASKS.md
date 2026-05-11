# Tasks: Mouse Support Follow-Up Bugs

See [BUG.md](BUG.md) for symptom analysis and root-cause traces.

## Wave 1: Implementation (3 parallel worktrees)

### Worktree A — Bug 1 (Mode buttons not clickable)

- [x] [Task 01](tasks/01-add-handle-set-mode-handler.md): Add `handle_set_mode` handler and wire `Message::NewSessionDialogSetMode` route — Done, validator PASS, merged in `c6e9c77`
- [x] [Task 02](tasks/02-register-mode-button-regions.md): Register per-button click regions in `launch_context.rs` *(depends on 01)* — Done, validator PASS, merged in `c6e9c77`

### Worktree B — Bugs 2 & 3 (both touch `terminal.rs`)

- [x] [Task 04](tasks/04-teardown-reorder-and-drain.md): Reorder teardown, move panic-hook install, add `drain_input` helper — Done, validator PASS, merged in `f24fa48`
- [x] [Task 03](tasks/03-osc22-pointer-shape.md): Emit OSC 22 pointer-shape sequences in `terminal.rs` *(depends on 04 — both touch `terminal.rs`)* — Done, validator PASS, merged in `f24fa48`

### Worktree C — Docs (independent)

- [x] [Task 05](tasks/05-docs-mouse-followup.md): Document OSC 22 caveat and manual mouse-exit verification step — Done, validator PASS, merged in `3cc6a08`

## Wave 2: Build Verification

- [x] [Task 06](tasks/06-build-verification.md): Full workspace `fmt --check`, `check`, `test`, `clippy` *(depends on all)* — Done, validator PASS (4,327 tests passed / 0 failed / 105 ignored)

---

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read |
|---|---|---|
| 01 | `crates/fdemon-app/src/handler/new_session/launch_context.rs`, `crates/fdemon-app/src/handler/update.rs` | `crates/fdemon-app/src/message.rs` |
| 02 | `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` | Task 01's handler |
| 03 | `crates/fdemon-tui/src/terminal.rs` | — |
| 04 | `crates/fdemon-tui/src/runner.rs`, `crates/fdemon-tui/src/event.rs`, `crates/fdemon-tui/src/terminal.rs` | — |
| 05 | `docs/DEVELOPMENT.md` (1 line in "Common Issues"); add `docs/MOUSE.md` if it does not exist already, else amend it | — |
| 06 | none (verification only) | all |

### Overlap Matrix

| | 01 | 02 | 03 | 04 | 05 | 06 |
|---|---|---|---|---|---|---|
| **01** | — | dep → 02 | — | — | — | dep → 06 |
| **02** | dep ← 01 | — | — | — | — | dep → 06 |
| **03** | — | — | — | **shared write: `terminal.rs`** | — | dep → 06 |
| **04** | — | — | **shared write: `terminal.rs`** | — | — | dep → 06 |
| **05** | — | — | — | — | — | — |
| **06** | dep ← 01–05 | | | | | — |

### Strategy

- **Parallel (worktrees):** Tasks 01+02 (Worktree A), Tasks 04+03 (Worktree B), Task 05 (Worktree C) — disjoint write sets across worktrees.
- **Sequential (same branch within Worktree B):** Task 04 → Task 03 — both write `terminal.rs`. T4 lands the structural teardown + panic-hook reorder; T3 adds the OSC 22 emission inside the existing `enable_mouse_capture` / `disable_mouse_capture`.
- **Sequential (same branch within Worktree A):** Task 01 → Task 02 — T2 emits a message whose route is added by T1.
- **Wave 2:** Task 06 (full verification) after all worktrees merge.

`docs/DEVELOPMENT.md` is owned by `doc_maintainer`. Task 05 should be tagged
`Agent: doc_maintainer`.
