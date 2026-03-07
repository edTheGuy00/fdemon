## Task: Update Documentation for Phase 4 Features

**Objective**: Update IDE_SETUP.md and ARCHITECTURE.md to reflect the Phase 4 features: debug event flow, hot reload/restart custom requests, conditional breakpoints, logpoints, expression evaluation, source references, multi-session debugging, and production hardening.

**Depends on**: 02-hot-reload-restart-dap

**Estimated Time**: 2–3 hours

### Scope

- `docs/IDE_SETUP.md`: Update debugging instructions with new features
- `docs/ARCHITECTURE.md`: Update DAP server architecture section with event flow diagram
- `workflow/plans/features/dap-server/PLAN.md`: Mark Phase 4 acceptance criteria

### Details

#### IDE_SETUP.md Updates

1. **Debug event flow**: Document that stopped/continued/thread events now work correctly
2. **Hot reload/restart**: Document the `hotReload` and `hotRestart` custom DAP requests
3. **Conditional breakpoints**: Document that conditions and hit conditions are supported
4. **Logpoints**: Document the `{expression}` interpolation syntax
5. **Expression evaluation**: Document hover, watch, repl, and clipboard contexts
6. **Source references**: Document that SDK sources are viewable in the IDE
7. **Multi-session debugging**: Document how thread IDs map to sessions
8. **Troubleshooting**: Add section for common issues (timeouts, connection failures, stale configs)

#### ARCHITECTURE.md Updates

1. **Debug event flow diagram**: Show the complete path from VM Service → TEA handler → DAP adapter → IDE
2. **Channel architecture**: Document the `dap_debug_senders` registry pattern
3. **Breakpoint persistence**: Document the desired/active breakpoint separation
4. **Multi-session thread model**: Document thread ID namespacing

#### Zed-Specific Documentation

Document the Zed-specific behavior:
- How the play/pause button state depends on `stopped`/`continued` events
- Zed's `debugger.log_dap_communications` setting for debugging
- Current limitations of Zed's DAP client

### Acceptance Criteria

1. IDE_SETUP.md accurately describes all Phase 4 features
2. ARCHITECTURE.md includes the debug event flow diagram
3. No stale or misleading documentation (e.g., NoopBackend references removed)
4. Troubleshooting section covers common failure modes
5. All documented features are actually implemented

### Notes

- Only update documentation for features that are actually implemented and working. If a feature is deferred, do not document it.
- Remove any Phase 3 leftover documentation that mentions NoopBackend or non-functional states.
- Keep documentation concise — developers should be able to get started quickly.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `docs/IDE_SETUP.md` | Added "Phase 4 Debugging Features" section covering all 9 implemented features. Updated capabilities table to reflect implemented features. Expanded troubleshooting section with 5 new entries. Removed stale "planned for Phase 4" notes. Updated Phase 5 reference (was "Phase 3 covers manual configuration only"). |
| `docs/ARCHITECTURE.md` | Added full "DAP Server Subsystem" section with architecture overview, debug event flow diagram, `dap_debug_senders` channel architecture, breakpoint state model, multi-session thread ID namespacing table, coordinated pause flow, custom DAP events, and `DebugBackend` trait diagram. Added `fdemon-dap` crate to project structure tree, layered architecture table, dependency flow, module reference, and API surface. Updated workspace crate count from 4 to 5. |
| `workflow/plans/features/dap-server/PLAN.md` | Marked Phase 4 acceptance criteria items as done/planned per implementation status. |

### Notable Decisions/Tradeoffs

1. **NoopBackend retained in ARCHITECTURE.md**: The task said to remove stale NoopBackend references from user-facing docs. `NoopBackend` is a legitimate test helper in the codebase (used in `fdemon-dap` unit tests); it was only removed from `IDE_SETUP.md` where it was misleadingly described as a limitation rather than a test utility. It is now listed correctly in the `fdemon-dap` API surface as a test-only backend.

2. **Tasks 10 and 11 not documented as features**: Neither task had a completion summary in the task file and the source code showed no implementation (no `desired_breakpoints` field, no timeout/rate-limiting code). These were documented in IDE_SETUP.md only as "Planned" in the capability table, and in PLAN.md as `[ ]` items with notes.

3. **Debug event flow diagram placement**: The debug event flow diagram was placed in ARCHITECTURE.md under the new "DAP Server Subsystem" section rather than inside the existing "Data Flow" section. This keeps all DAP-specific architecture in one browsable section.

### Testing Performed

- `cargo check --workspace` — Passed (docs-only change, no source modifications)

### Risks/Limitations

1. **ARCHITECTURE.md project structure tree**: The `fdemon-dap` crate was added to the file tree. If the actual directory layout differs from what was documented, the tree will need updating. The layout was verified against the actual source files found via `find`.

2. **Breakpoint persistence note**: The troubleshooting section documents that breakpoints need to be re-set after hot restart. This matches the current behavior (Task 10 not implemented). When Task 10 lands, remove this note.
