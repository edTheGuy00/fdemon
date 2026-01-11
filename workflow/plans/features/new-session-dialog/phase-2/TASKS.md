# Phase 2: Fuzzy Search Modal - Task Index

## Overview

Create a reusable fuzzy search modal widget that appears as an overlay. Used for Configuration and Flavor selection with type-to-filter and custom input support.

**Total Tasks:** 4
**Estimated Time:** 2 hours

## UI Design

```
┌── Sidecar ──────────────────────────────────────────────────────────────┐
│  ┌── 🎯 Target Selector ─────────────┐ ┌── ⚙️ Launch Context ───────┐  │
│  │                                   │ │                             │  │
│  │  ... (Background Dimmed) ...      │ │  Flavor:                    │  │
│  │                                   │ │  [ dev __________________ ▼]│  │
│  │                                   │ │                             │  │
│  └───────────────────────────────────┘ └─────────────────────────────┘  │
│ ┌─────────────────────────────────────────────────────────────────────┐ │
│ │  🔍 Select Flavor (Type to filter)                                  │ │
│ ├─────────────────────────────────────────────────────────────────────┤ │
│ │ > dev_staging                                                       │ │
│ │   dev_production                                                    │ │
│ │   uat_testing                                                       │ │
│ │   uatb_testing                                                      │ │
│ │   prod_eu                                                           │ │
│ │   prod_us                                                           │ │
│ │   prod_asia                                                         │ │
│ └─────────────────────────────────────────────────────────────────────┘ │
│                                                                         │
│  [↑↓] Navigate  [Enter] Select  [Esc] Cancel  Type to filter/custom     │
└─────────────────────────────────────────────────────────────────────────┘
```

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-fuzzy-modal-state               │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  02-fuzzy-filter-algorithm          │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  03-fuzzy-modal-widget              │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  04-fuzzy-modal-messages            │
└─────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-fuzzy-modal-state](tasks/01-fuzzy-modal-state.md) | Not Started | Phase 1 | 25m | `new_session_dialog/state.rs` |
| 2 | [02-fuzzy-filter-algorithm](tasks/02-fuzzy-filter-algorithm.md) | Not Started | 1 | 20m | `new_session_dialog/fuzzy_modal.rs` |
| 3 | [03-fuzzy-modal-widget](tasks/03-fuzzy-modal-widget.md) | Not Started | 2 | 40m | `new_session_dialog/fuzzy_modal.rs` |
| 4 | [04-fuzzy-modal-messages](tasks/04-fuzzy-modal-messages.md) | Not Started | 3 | 15m | `app/message.rs`, `app/handler/update.rs` |

## Success Criteria

Phase 2 is complete when:

- [ ] `FuzzyModalState` struct with query, items, filtered_indices, selected_index
- [ ] Fuzzy/substring matching algorithm implemented
- [ ] Modal renders as overlay with dimmed background
- [ ] Type-to-filter updates list in real-time
- [ ] Up/Down navigation with wrapping
- [ ] Enter selects item OR uses custom query text
- [ ] Esc cancels and closes modal
- [ ] Message handlers wired up
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

## Behavior Specification

### Opening the Modal
- Triggered by pressing Enter on Config or Flavor field in Launch Context
- Modal type determines title and item source:
  - `Config` → "Select Configuration", items from `LoadedConfigs`
  - `Flavor` → "Select Flavor", items from project analysis + custom input

### Filtering
- Empty query shows all items
- Typing updates `query` and filters items in real-time
- Matching is case-insensitive
- Support both substring match and fuzzy match (characters in order)
- Filter preserves original order of matched items

### Navigation
- Up/Down moves selection within filtered results
- Wraps around (bottom → top, top → bottom)
- Selection resets to 0 when filter changes

### Selection
- Enter with filtered items → select highlighted item
- Enter with no matches + non-empty query → use query as custom value (Flavor only)
- For Config modal, custom input not allowed (must select from list)

### Cancellation
- Esc closes modal without changing value
- Returns focus to the field that triggered the modal

## Notes

- Modal state is stored in `NewSessionDialogState.fuzzy_modal: Option<FuzzyModalState>`
- When `fuzzy_modal.is_some()`, main dialog input is blocked
- Consider scroll offset for long lists (visible window)
