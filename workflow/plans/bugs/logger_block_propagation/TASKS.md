# Logger Block Propagation & Performance - Task Index

## Overview

This bug fix addresses inconsistent coloring of Logger package blocks and high CPU usage during log processing. The plan consists of 5 tasks ordered by priority and dependency.

**Total Tasks:** 5
**Estimated Total Effort:** 14-19 hours

## Task Dependency Graph

```
┌─────────────────────────────────────────────────────────────────┐
│                        PRIORITY ORDER                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────────────┐    ┌──────────────────────┐          │
│  │ 01-stateful-block    │    │ 02-false-positive    │          │
│  │     (HIGH)           │    │     (MEDIUM)         │          │
│  │                      │    │                      │          │
│  │ Fixes O(N*M) perf    │    │ Fixes ErrorTestPage  │          │
│  │ + block propagation  │    │ false positives      │          │
│  └──────────┬───────────┘    └──────────────────────┘          │
│             │                         (parallel)                │
│             ▼                                                   │
│  ┌──────────────────────┐                                      │
│  │ 03-ring-buffer       │                                      │
│  │     (LOW)            │                                      │
│  │                      │                                      │
│  │ Caps memory usage    │                                      │
│  └──────────┬───────────┘                                      │
│             │                                                   │
│             ├─────────────────────┐                            │
│             ▼                     ▼                            │
│  ┌──────────────────────┐  ┌──────────────────────┐           │
│  │ 04-coalesce-updates  │  │ 05-virtualized       │           │
│  │     (LOW)            │  │     (LOW)            │           │
│  │                      │  │                      │           │
│  │ Batch log arrivals   │  │ Render only visible  │           │
│  └──────────────────────┘  └──────────────────────┘           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Priority | Depends On | Effort | Modules |
|---|------|--------|----------|------------|--------|---------|
| 01 | [stateful-block-tracking](tasks/01-stateful-block-tracking.md) | ✅ Done | HIGH | - | 3-4h | `session.rs` |
| 02 | [fix-false-positive-detection](tasks/02-fix-false-positive-detection.md) | ✅ Done | MEDIUM | - | 2-3h | `protocol.rs`, `helpers.rs`, `ansi.rs` |
| 03 | [ring-buffer-log-storage](tasks/03-ring-buffer-log-storage.md) | ✅ Done | LOW | 01 | 2-3h | `session.rs`, `types.rs`, `log_view.rs`, `render.rs` |
| 04 | [coalesce-rapid-updates](tasks/04-coalesce-rapid-updates.md) | ✅ Done | LOW | 01 | 3-4h | `daemon.rs`, `session.rs`, `runner.rs` |
| 05 | [virtualized-log-display](tasks/05-virtualized-log-display.md) | ✅ Done | LOW | 01, 03 | 4-5h | `log_view.rs`, `session.rs` |

## Technical Note: The `error: true` Flag

The `app.log` event from Flutter daemon includes an `error: bool` field:

```json
{"event":"app.log","params":{"appId":"abc","log":"message","error":true}}
```

- **`error: true`** (stderr) → Already handled correctly in `parse_flutter_log()` - returns `LogLevel::Error` immediately
- **`error: false`** (stdout) → Falls back to content-based detection - **this is where false positives occur**

This means Task 02 only needs to fix content-based detection for stdout logs. True errors from stderr are already correctly identified.

## Execution Order

### Phase A: Critical Fixes (Tasks 01 & 02)
These can be worked on in parallel:

1. **Task 01: Stateful Block Tracking** - Most important
   - Fixes O(N*M) → O(B) performance
   - Enables correct block-level propagation
   - Foundation for Tasks 03, 04, 05

2. **Task 02: False Positive Detection** - Independent
   - Fixes "ErrorTestingPage" false positive
   - Simple word boundary fix
   - Can be done anytime

### Phase B: Memory Optimization (Task 03)
After Task 01 is complete:

3. **Task 03: Ring Buffer** - Recommended
   - Caps memory growth
   - Required before Task 05
   - Low effort, high value

### Phase C: Performance Polish (Tasks 04 & 05)
Only if CPU usage remains high after Phase A & B:

4. **Task 04: Coalesce Updates** - Optional
   - Reduces render frequency
   - Independent of Task 05

5. **Task 05: Virtualized Display** - Optional
   - Final optimization
   - Most complex task
   - Only needed for 10,000+ log scenarios

## Success Criteria

After completing Tasks 01-03 (minimum viable):
- [ ] Logger blocks display with consistent coloring
- [ ] No false positives on class names like `ErrorTestingPage`
- [ ] No O(N*M) backward scanning on block end
- [ ] Memory usage capped at configurable limit
- [ ] CPU usage acceptable during normal logging

After completing all tasks:
- [ ] Smooth UI during high-volume logging bursts
- [ ] Responsive scrolling with 50,000+ logs
- [ ] ~60fps render rate maintained under load

## Related Documentation

- [BUG.md](BUG.md) - Full bug analysis and research findings
- [Task 08: Strip ANSI Codes](../../../features/log-config-enhancements/phase_2/tasks/08-strip-ansi-escape-codes.md) - Completed dependency
- [Task 09: Log Level Detection](../../../features/log-config-enhancements/phase_2/tasks/09-enhance-log-level-detection.md) - Completed dependency
- [Task 11: Block Level Propagation](../../../features/log-config-enhancements/phase_2/tasks/11-logger-block-level-propagation.md) - Original implementation (being replaced)
