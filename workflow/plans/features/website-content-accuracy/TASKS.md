# Website Content Accuracy & Multi-Launch Documentation - Task Index

## Overview

Document the shipped multi-device launch / UX-polish features on the website and fix the
~32 documentation-vs-codebase discrepancies surfaced by the sweep. All website tasks edit
distinct files under `website/src/`; three `doc_maintainer` tasks correct the canonical
`docs/*.md`.

**Total Tasks:** 9
**Estimated Hours:** 15-24 hours

Plan: [PLAN.md](./PLAN.md)

## Task Dependency Graph

```
All tasks are independent (no inter-task dependencies) — one parallel wave:

┌───────────────────┐ ┌───────────────────┐ ┌───────────────────┐
│ 01-data-keybind…  │ │ 02-configuration  │ │ 03-native-logs    │
└───────────────────┘ └───────────────────┘ └───────────────────┘
┌───────────────────┐ ┌───────────────────┐ ┌───────────────────┐
│ 04-devtools       │ │ 05-small-page-fix │ │ 06-architecture   │
└───────────────────┘ └───────────────────┘ └───────────────────┘
┌───────────────────┐ ┌───────────────────┐ ┌───────────────────┐
│ 07-docs-config-md │ │ 08-docs-keybind-md│ │ 09-docs-arch-md   │
└───────────────────┘ └───────────────────┘ └───────────────────┘
        (implementor)          (implementor)          (doc_maintainer ×3: 07–09)

Cross-plan: the SEO plan's S05 (leptos_meta) and S09 (landing copy) edit the same
page files as T01/T05/T06 and must run AFTER this plan lands.
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Agent | Modules |
|---|------|--------|------------|------------|-------|---------|
| 1 | [01-data-keybindings-features](tasks/01-data-keybindings-features.md) | ✅ Done | - | 3-4h | implementor | `website/src/data.rs` |
| 2 | [02-configuration-page](tasks/02-configuration-page.md) | ✅ Done | - | 2-3h | implementor | `website/src/pages/docs/configuration.rs` |
| 3 | [03-native-logs-page](tasks/03-native-logs-page.md) | ✅ Done | - | 2-3h | implementor | `website/src/pages/docs/native_logs.rs` |
| 4 | [04-devtools-page](tasks/04-devtools-page.md) | ✅ Done | - | 2-3h | implementor | `website/src/pages/docs/devtools.rs` |
| 5 | [05-small-page-fixes](tasks/05-small-page-fixes.md) | ✅ Done | - | 1-2h | implementor | `website/src/pages/docs/{mouse,installation,introduction}.rs` |
| 6 | [06-architecture-page](tasks/06-architecture-page.md) | ✅ Done | - | 2-3h | implementor | `website/src/pages/docs/architecture.rs` |
| 7 | [07-docs-configuration-md](tasks/07-docs-configuration-md.md) | ✅ Done | - | 1-2h | doc_maintainer | `docs/CONFIGURATION.md` |
| 8 | [08-docs-keybindings-md](tasks/08-docs-keybindings-md.md) | ✅ Done | - | 1-2h | doc_maintainer | `docs/KEYBINDINGS.md` |
| 9 | [09-docs-architecture-md](tasks/09-docs-architecture-md.md) | ✅ Done (no drift; verified) | - | 1-2h | doc_maintainer | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

<!-- The orchestrator uses this section to determine isolation strategy per wave -->

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|----------------------------|
| 01-data-keybindings-features | `website/src/data.rs` | `crates/fdemon-app/src/handler/keys.rs`, `new_session_dialog/*`, `docs/KEYBINDINGS.md` |
| 02-configuration-page | `website/src/pages/docs/configuration.rs` | `crates/fdemon-app/src/config/types.rs` |
| 03-native-logs-page | `website/src/pages/docs/native_logs.rs` | `crates/fdemon-app/src/config/types.rs`, `crates/fdemon-daemon/src/native_logs/custom.rs` |
| 04-devtools-page | `website/src/pages/docs/devtools.rs` | `crates/fdemon-app/src/handler/keys.rs` |
| 05-small-page-fixes | `website/src/pages/docs/mouse.rs`, `.../installation.rs`, `.../introduction.rs` | `Cargo.toml`, `keys.rs`, `crates/fdemon-core/src/types.rs` |
| 06-architecture-page | `website/src/pages/docs/architecture.rs` | `Cargo.toml`, `docs/ARCHITECTURE.md`, `crates/` |
| 07-docs-configuration-md | `docs/CONFIGURATION.md` | `crates/fdemon-app/src/config/types.rs` |
| 08-docs-keybindings-md | `docs/KEYBINDINGS.md` | `crates/fdemon-app/src/handler/keys.rs` |
| 09-docs-architecture-md | `docs/ARCHITECTURE.md` | `Cargo.toml`, `crates/` |

### Overlap Matrix

<!-- Read-only overlap is fine — only write overlap forces sequential execution -->

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|--------------------|
| Any pair among 01–09 | None | Parallel (worktree) |

Every task writes a unique file (05 writes three files written by no other task). No two
tasks share a write file → all 9 are safe to run concurrently in isolated worktrees.

## Success Criteria

This feature is complete when:

- [x] All 20 HIGH-severity discrepancies are corrected, each traceable to a `crate/file:line`.
- [x] The multi-device launch picker is documented (`Space`/`a`/`Enter`/`r` + footer hint).
- [x] Launch-lifecycle phases (`Preparing`/`Launching`/`Running`) and the jump-to-latest
      indicator are documented.
- [x] native_logs TOML examples parse against `NativeLogsSettings`; the new "Boot your
      whole stack" orchestrator section is present and accurate.
- [x] DevTools docs include the Memory panel and drop the fabricated Layout Explorer key.
- [x] Architecture page reflects the real 5-crate workspace.
- [x] Installation page states Rust `1.77.2`.
- [x] `cd website && cargo check` succeeds; changed pages render. (`trunk build`/wasm
      toolchain unavailable in this environment — `cargo check` is the type-correctness gate.)
- [x] `doc-validate` content boundaries pass for each edited `docs/*.md` (07/08 fixed real
      drift; 09 verified already-accurate, no changes needed).

## Notes

- Tasks 07–09 are routed to `doc_maintainer` (core-doc content boundaries). They
  verify-and-fix only real drift — research indicated the markdown docs are largely the
  correct source the website lagged.
- No Rust app crates (`crates/*`) are modified, so the workspace test suite is unaffected.
- Cross-plan ordering with the SEO plan is enforced in the SEO `TASKS.md`, not here.
</content>
