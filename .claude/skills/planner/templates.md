# Planning Document Templates

Reference templates for creating plans, bug reports, task indexes, and individual tasks.

---

## Feature Plan Template

**Location:** `workflow/plans/features/<feature-name>/PLAN.md`

```markdown
# Plan: <Feature Name>

## TL;DR

<20-100 words: what/how/why>

---

## Background

<Context on why this feature is needed, current limitations, user pain points>

---

## Affected Modules

- `src/<module>.rs` - <Brief description of changes>
- `src/<module>.rs` - **NEW** <For new files>

---

## Development Phases

### Phase 1: <Phase Title>

**Goal**: <1-2 sentence description>

**Duration**: <Estimate>

#### Steps

1. **<Step Title>**
   - <Bullet points with specific changes>
   - <Implementation details>

2. **<Step Title>**
   - <Details>

**Milestone**: <What users can do when phase complete>

---

### Phase 2: <Phase Title>

<Repeat structure>

---

## Edge Cases & Risks

### <Risk Category>
- **Risk:** <Description>
- **Mitigation:** <How to address>

---

## Configuration Additions

<If applicable, show config file additions>

```toml
[section]
option = "value"
```

---

## Keyboard Shortcuts Summary

| Key | Action |
|-----|--------|
| `x` | <Action> |

---

## Success Criteria

### Phase 1 Complete When:
- [ ] <Measurable outcome>
- [ ] <Testable condition>

### Phase 2 Complete When:
- [ ] <Measurable outcome>

---

## Future Enhancements

<Optional: ideas for future iterations>

---

## References

- [Link](url)
```

---

## Bug Report Plan Template

**Location:** `workflow/plans/bugs/<bug-name>/BUG.md`

```markdown
# Bugfix Plan: <Bug Title>

## TL;DR

<20-100 words: what bugs exist, root cause summary, fix approach>

## Bug Reports

### Bug 1: <Bug Title>
**Symptom:** <What the user sees>

**Expected:** <What should happen>

**Root Cause Analysis:**
1. <Specific code path causing issue>
2. <Why it fails>

**Affected Files:**
- `src/<file>.rs` - <description>

---

### Bug 2: <Bug Title>

<Repeat structure>

---

## Affected Modules

- `src/<module>.rs`: <Description of needed changes>

---

## Phases

### Phase 1: <Fix Category> (Bug X) - Critical

<Description of fix approach>

**Steps:**
1. <Specific code change>
2. <Implementation detail>

**Measurable Outcomes:**
- <How to verify fix works>
- <Test case>

---

## Edge Cases & Risks

### <Risk Category>
- **Risk:** <Description>
- **Mitigation:** <How to address>

---

## Further Considerations

1. **<Question>** <Options or tradeoffs>

---

## Task Dependency Graph

```
Phase 1
├── 01-task-slug
├── 02-task-slug
│   └── depends on: 01
└── 03-task-slug
    └── depends on: 02
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] <Bug X is fixed, verified by...>
- [ ] <No regression in...>

---

## Milestone Deliverable

<Summary of what's achieved when all bugs are fixed>
```

---

## Task Index Template

**Location:** `workflow/plans/<type>/<name>/<phase>/TASKS.md`

```markdown
# <Phase/Feature Name> - Task Index

## Overview

<1-2 sentence summary of this phase/feature>

**Total Tasks:** X
**Estimated Hours:** X-Y hours

## Task Dependency Graph

```
┌─────────────────────┐     ┌─────────────────────┐
│  01-task-slug       │     │  02-task-slug       │
└─────────┬───────────┘     └──────────┬──────────┘
          │                            │
          └──────────┬─────────────────┘
                     ▼
          ┌─────────────────────┐
          │  03-task-slug       │
          └─────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-task-slug](tasks/01-task-slug.md) | Not Started | - | 3-4h | `module.rs` |
| 2 | [02-task-slug](tasks/02-task-slug.md) | Not Started | - | 2-3h | `module.rs` |
| 3 | [03-task-slug](tasks/03-task-slug.md) | Not Started | 1, 2 | 4-5h | `module.rs` |

## Success Criteria

<Phase/feature> is complete when:

- [ ] <Measurable outcome>
- [ ] <Testable condition>
- [ ] All new code has unit tests
- [ ] No regressions in existing functionality

## Keyboard Shortcuts

<If applicable>

| Key | Action |
|-----|--------|
| `x` | <Action> |

## Notes

- <Important context>
- <Constraints or considerations>
```

---

## Individual Task Template

**Location:** `workflow/plans/<type>/<name>/<phase>/tasks/<##-task-slug>.md`

```markdown
## Task: <Task Title>

**Objective**: <1-2 sentences describing what this task accomplishes>

**Depends on**: <Task slugs or "None">

**Estimated Time**: <X-Y hours>

### Scope

- `src/<module>.rs`: <Specific changes to make>

### Details

<Detailed implementation guidance, code examples if helpful>

```rust
// Example code structure
pub struct Example {
    pub field: Type,
}
```

### Acceptance Criteria

1. <Measurable outcome - can be verified>
2. <Testable condition - can be unit tested>
3. <Behavior specification>

### Testing

<Test approach and example test cases>

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example() {
        // Test implementation
    }
}
```

### Notes

- <Important considerations>
- <Edge cases to handle>
- <Future enhancements to defer>

---

## Completion Summary

**Status:** ✅ Done / ⚠️ Blocked / ❌ Not done

**Files Modified:**
- `src/<file>.rs` - <what changed>

**Implementation Details:**

<Brief summary of how it was implemented, key decisions>

**Testing Performed:**
- `cargo fmt` - Passed/Failed
- `cargo check` - Passed/Failed
- `cargo clippy -- -D warnings` - Passed/Failed
- `cargo test <module>` - X tests passed

**Notable Decisions:**
- <Any deviations from plan or architectural choices>

**Risks/Limitations:**
- <Known issues or future work needed>
```

---

## Status Icons Reference

| Status | Icon |
|--------|------|
| Not Started | (blank) |
| In Progress | 🔄 |
| Done | ✅ |
| Blocked | ⚠️ |
| Not Done / Failed | ❌ |

## File Naming Conventions

- **Plans:** `PLAN.md` (features) or `BUG.md` (bugs)
- **Task Index:** `TASKS.md`
- **Tasks:** `##-task-slug.md` (e.g., `01-add-filter-types.md`)
- **Slugs:** lowercase, hyphen-separated, descriptive
