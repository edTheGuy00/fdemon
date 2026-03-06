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
