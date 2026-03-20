## Task: Update Documentation for Lazy-Start Monitoring

**Agent:** doc_maintainer

**Objective**: Update core project documentation to reflect the change from eager to lazy performance monitoring startup.

**Depends on**: 03-lazy-start-monitoring

**Estimated Time**: 0.5 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`: Update the DevTools Subsystem section to reflect lazy-start behavior and panel-aware pause/resume

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md`: Content boundary rules
- `workflow/plans/bugs/profile-mode-lag/phase3/tasks/01-pause-perf-when-not-devtools.md`: perf_pause mechanism
- `workflow/plans/bugs/profile-mode-lag/phase3/tasks/02-pause-network-on-tab-switch.md`: network_pause mechanism
- `workflow/plans/bugs/profile-mode-lag/phase3/tasks/03-lazy-start-monitoring.md`: Lazy-start change context

### Change Context

1. **DevTools Subsystem — VM Service Data Flow** (ARCHITECTURE.md): The current text states "Engine spawns background polling tasks (performance monitor, network monitor) when a session connects." After Phase 3, performance monitoring starts when the user first enters DevTools, not on session connect. Network monitoring was already demand-started. Update this sentence to reflect the lazy/demand-start behavior.

2. **DevTools Subsystem — Panel State Model** (ARCHITECTURE.md): The description of per-session state should mention the new pause channels (`perf_pause_tx`, `network_pause_tx`) alongside the existing `alloc_pause_tx`, and note that monitoring is panel-gated.

3. **Session Handle fields** (ARCHITECTURE.md): The `SessionHandle` diagram should include `perf_pause_tx` and `network_pause_tx` fields.

### Acceptance Criteria

1. ARCHITECTURE.md accurately describes the lazy-start behavior for performance monitoring
2. ARCHITECTURE.md accurately describes the panel-aware pause/resume for both performance and network monitoring
3. The `SessionHandle` diagram includes `perf_pause_tx` and `network_pause_tx`
4. No content boundary violations
5. Changes are minimal and targeted — do not rewrite the entire DevTools section

### Notes

- Follow content boundaries strictly — architecture content only in ARCHITECTURE.md
- The key change is behavioral: monitoring lifecycle went from "eager start at VM connect" to "lazy start on first DevTools entry" for performance, with pause/resume gating for both performance and network
- Network monitoring was already demand-started (on first Network tab visit). The new behavior is that it pauses when leaving the Network tab and resumes when returning.
