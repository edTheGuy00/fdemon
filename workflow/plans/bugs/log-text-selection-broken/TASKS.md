# TASKS — Log text-selection / copy bug fix

Parent plan: [BUG.md](./BUG.md)

This bug ships as one PR — every task targets the same fix and the doc updates that ride with it. Tasks are partitioned by file so they can run in parallel worktrees where possible.

## Tasks

| # | Title | Wave | Depends on | Agent | File | Status |
|---|-------|------|------------|-------|------|--------|
| 01 | Drop `?1003` + add runtime `set_mouse_capture` | 1 | — | implementor | [tasks/01-drop-1003-and-runtime-toggle.md](./tasks/01-drop-1003-and-runtime-toggle.md) | [x] Done (CONCERN: doc-comment for `set_mouse_capture` overpromises error surfacing on disable path; functional core correct) |
| 02 | Clipboard service trait + arboard impl + memory mock | 1 | — | implementor | [tasks/02-clipboard-service.md](./tasks/02-clipboard-service.md) | [x] Done |
| 03 | New `Message` variants, `UpdateAction` variant, `AppState` field | 1 | — | implementor | [tasks/03-messages-and-state.md](./tasks/03-messages-and-state.md) | [x] Done |
| 04 | Right-click on log row → copy line; non-log → toast | 2 | 02, 03 | implementor | [tasks/04-right-click-copy.md](./tasks/04-right-click-copy.md) | [x] Done |
| 05 | `Alt+m` toggle binding → `Message::ToggleMouseCapture` | 2 | 03 | implementor | [tasks/05-alt-m-toggle-binding.md](./tasks/05-alt-m-toggle-binding.md) | [x] Done |
| 06 | `handler/update.rs` arms for the three new messages | 2 | 02, 03 | implementor | [tasks/06-update-handler-arms.md](./tasks/06-update-handler-arms.md) | [x] Done (CONCERN: `resolve_entry_text` exercised via two tests rather than a standalone focused test; merge conflict with task 05 in `tests.rs` + `terminal.rs` resolved manually) |
| 07 | Runner glue: observe `UpdateAction::SetMouseCapture`, follow-up event | 3 | 01, 03, 06 | implementor | [tasks/07-runner-side-effect.md](./tasks/07-runner-side-effect.md) | [x] Done (CONCERN: out-of-scope additions to fdemon-app for `pending_runner_actions` queue + `Engine::drain_runner_actions` were necessary supporting infrastructure not declared by prior tasks) |
| 08 | Status-bar mouse indicator via `StatusInfo` | 3 | 03 | implementor | [tasks/08-status-indicator.md](./tasks/08-status-indicator.md) | [x] Done (minor: one vacuous test assertion noted, not blocking) |
| 09 | Update non-core docs (MOUSE / KEYBINDINGS / CONFIGURATION / PLAN cross-ref) | 4 | 01–08 | implementor | [tasks/09-non-core-docs.md](./tasks/09-non-core-docs.md) | [x] Done (CONCERN: BUG.md reference in PLAN.md is a backtick prose path rather than a clickable markdown hyperlink; `cargo fmt --all` produced a whitespace-only edit to `widgets/log_view/mod.rs`) |
| 10 | Update `docs/ARCHITECTURE.md` for new service + update channel | 4 | 02, 03, 07 | **doc_maintainer** | [tasks/10-architecture-doc.md](./tasks/10-architecture-doc.md) | [x] Done (CONCERN: `SetMouseCapture` description on line 1666 mentions `?1003` DECSET — terminal-protocol detail explicitly out of scope for ARCHITECTURE.md per task spec; one-phrase deletion fixes it) |

Waves 1 → 4 must complete in order. Within a wave, all tasks may dispatch in parallel — see overlap matrix below.

---

## File Overlap Analysis

### Files Modified per Task (write set)

| Task | Files Written |
|------|---------------|
| 01 | `crates/fdemon-tui/src/terminal.rs` |
| 02 | `crates/fdemon-app/src/services/clipboard.rs` *(new)*, `crates/fdemon-app/src/services/mod.rs`, `crates/fdemon-app/Cargo.toml` |
| 03 | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/update_action.rs` *(or wherever `UpdateAction` lives — verify in task)* |
| 04 | `crates/fdemon-app/src/handler/mouse.rs` |
| 05 | `crates/fdemon-app/src/handler/keys.rs` |
| 06 | `crates/fdemon-app/src/handler/update.rs` |
| 07 | `crates/fdemon-tui/src/runner.rs` |
| 08 | `crates/fdemon-tui/src/widgets/log_view/mod.rs`, `crates/fdemon-tui/src/render/mod.rs` |
| 09 | `docs/MOUSE.md`, `docs/KEYBINDINGS.md`, `docs/CONFIGURATION.md`, `workflow/plans/features/mouse-support/PLAN.md` |
| 10 | `docs/ARCHITECTURE.md` |

### Files Read (read-only deps — no overlap risk)

- Tasks 04, 05, 06, 07, 08 read `message.rs`, `state.rs`, `update_action.rs` — fine because Task 03 must finish first (dependency); after that they're effectively immutable to the dependents.
- Tasks 04, 06 read `services/clipboard.rs` — same logic for Task 02.

### Overlap Matrix (wave-peers only)

| Pair | Shared write files? | Strategy |
|------|---------------------|----------|
| **Wave 1** | | |
| 01 ↔ 02 | none | Parallel (worktree) |
| 01 ↔ 03 | none | Parallel (worktree) |
| 02 ↔ 03 | none | Parallel (worktree) |
| **Wave 2** | | |
| 04 ↔ 05 | none | Parallel (worktree) |
| 04 ↔ 06 | none | Parallel (worktree) |
| 05 ↔ 06 | none | Parallel (worktree) |
| **Wave 3** | | |
| 07 ↔ 08 | none | Parallel (worktree) |
| **Wave 4** | | |
| 09 ↔ 10 | none — `ARCHITECTURE.md` is doc_maintainer-only; Task 09 must not touch it | Parallel (worktree) |

All wave-peer pairs are parallel-safe.

### Notes on Why Boundaries Land Here

- **Task 03 is a "platform" wave** — it adds the messages, action variants, and state field that the rest of the bug fix consumes. Splitting it further (e.g., one task per `Message` variant) would create false parallelism since `message.rs` is a single file.
- **Wave 2 fans out cleanly** because handler dispatch is already segmented into per-area files (`handler/mouse.rs`, `handler/keys.rs`, `handler/update.rs`).
- **Task 07's runner glue is its own task** because it crosses the layer boundary (consumes `UpdateAction`, calls into `fdemon-tui::terminal`). Mixing this with Task 06 (which lives entirely in the handler layer) would muddy the layering review.
- **Task 10 is routed to `doc_maintainer`** because `docs/ARCHITECTURE.md` is in the core-docs allow-list (managed by `doc_maintainer` only per planner.md guidance). Task 09 covers all the unmanaged docs.

---

## Verification (post-merge gate)

After all ten tasks complete, the implementor of Task 09 (or whoever finishes last) runs:

```bash
cargo fmt --all
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

Plus the manual-test matrix in `BUG.md` — minimum two terminals (one macOS, one Linux) before opening the PR.
