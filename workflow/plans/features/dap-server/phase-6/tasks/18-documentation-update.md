## Task: Update Documentation for Phase 6

**Agent:** doc_maintainer

**Objective**: Update core project documentation to reflect Phase 6 changes: new DAP capabilities, expanded backend trait, variable system overhaul, and new request handlers.

**Depends on**: 01-fix-variable-display-bugs through 17-request-timeouts-events (all implementation tasks)

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`: Update DAP Server Subsystem section with Phase 6 capabilities

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: Content boundary rules
- All Phase 6 task completion summaries

### Change Context

1. **DAP capabilities expanded**: New requests (`exceptionInfo`, `restartFrame`, `loadedSources`, `callService`, `updateDebugOptions`, `breakpointLocations`, `completions`, `restart`) and their capability flags
2. **Backend trait expanded**: `get_isolate`, `call_service`, `set_library_debuggable`, `get_source_report` methods added
3. **Variable system overhaul**: Globals scope, exception scope, getter evaluation, `toString()` display, `evaluateName` construction, Record/WeakReference/Sentinel type support
4. **New adapter state**: `evaluate_getters_in_debug_views`, `evaluate_to_string_in_debug_views`, `debug_sdk_libraries`, `debug_external_package_libraries`, `exception_refs`, `first_async_marker_index`, `client_supports_progress`
5. **Request timeout enforcement**: All backend calls wrapped with `REQUEST_TIMEOUT`
6. **Custom events**: `dart.serviceExtensionAdded`, `dart.hotReloadComplete`, `dart.hotRestartComplete`, progress events

### Acceptance Criteria

1. Updated docs accurately reflect Phase 6 implementation
2. No content boundary violations
3. All required sections present per schemas.md
4. DAP Server Subsystem section lists all implemented DAP requests

### Notes

- Follow content boundaries strictly — architecture content only in ARCHITECTURE.md
- Make targeted edits, do not rewrite entire documents
- The existing DAP Server Subsystem section in ARCHITECTURE.md should be expanded, not replaced
