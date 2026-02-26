## Task: Correct inaccurate claim in task-03 completion notes

**Objective**: Fix the factually incorrect statement in the phase-4 task-03 notes that claims "stopped sessions have `app_id = None`". The `mark_stopped()` method does NOT clear `app_id`.

**Depends on**: None

### Scope

- `workflow/plans/features/session-resilience/phase-4/tasks/03-update-launch-guard.md`: Fix line 71

### Details

#### Current (line 71):

```
- `find_by_app_id` is not affected — stopped sessions have `app_id = None`.
```

#### Corrected:

```
- `find_by_app_id` is not affected — it routes daemon events by `app_id`, which is a separate concern from device-level duplicate prevention.
```

**Why the original is wrong:** `mark_stopped()` (session.rs:462–464) only sets `self.phase = AppPhase::Stopped`. It does NOT clear `app_id`. A session that ran and was stopped retains its `app_id`. The conclusion is correct (no impact on `find_by_app_id`), but the reasoning is factually wrong.

### Acceptance Criteria

1. Line 71 of task-03 is corrected to state the actual reason `find_by_app_id` is unaffected
2. No other files modified

### Testing

No code changes — workflow documentation fix only.

### Notes

- This is a documentation-only fix in the workflow plans directory. No source code affected.
