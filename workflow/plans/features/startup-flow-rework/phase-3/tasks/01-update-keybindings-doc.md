## Task: Update KEYBINDINGS.md Documentation

**Objective**: Update the keybindings documentation to reflect the new '+' keybinding and remove references to 'n' for session management.

**Depends on**: None (but should be done after Phase 2)

### Scope

- `docs/KEYBINDINGS.md`: Multiple sections need updates

### Details

**1. Update Session Management section (around line 45-56):**

Current:
```markdown
| `d` | Add Device Session | Add device session (shows Startup Dialog if no sessions, Device Selector if sessions running) |
| `n` | Device Selector | Alternative binding (also used for search navigation) |
```

Change to:
```markdown
| `+` | Start New Session | Start a new session (shows Startup Dialog if no sessions, Device Selector if sessions exist) |
| `d` | Start New Session | Alternative binding for starting new session |
```

**2. Update Log Search section (around line 110-115):**

Current:
```markdown
| `/` | Start Search | Enter search input mode to type a query |
| `n` | Next Match | Jump to the next search match |
| `N` | Previous Match | Jump to the previous search match |

> **Note:** The `n` key is context-sensitive. If a search query is active, it navigates to the next match. Otherwise, it opens the device selector.
```

Change to:
```markdown
| `/` | Start Search | Enter search input mode to type a query |
| `n` | Next Match | Jump to the next search match (only when search active) |
| `N` | Previous Match | Jump to the previous search match |
```

Remove the "Note" about context-sensitive behavior.

**3. Update Tips section (around line 346-352):**

Current text references 'n' key, update any references.

**4. Add information about "Not Connected" state:**

Add a new section or note in the Normal Mode section:

```markdown
### Startup State

When Flutter Demon starts without auto-start configured, you'll see:
- Status bar: "○ Not Connected"
- Log area: "Press + to start a new session"

Press `+` or `d` to open the Startup Dialog and configure your first session.
```

### Acceptance Criteria

1. '+' key documented in Session Management section
2. 'd' key described as alternative to '+'
3. 'n' key only documented for search navigation
4. No "context-sensitive" note about 'n' key
5. "Not Connected" startup state documented
6. All markdown renders correctly

### Testing

```bash
# Check for any remaining references to 'n' for sessions
grep -n "'n'" docs/KEYBINDINGS.md
# Should only show search-related entries

# Verify markdown renders (optional)
# Use a markdown preview tool or GitHub
```

### Notes

- Keep the markdown table formatting consistent
- Ensure the description fits well in the table cell
- Consider adding a "What's New" or "Breaking Changes" note at the top if this is a significant change

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (To be filled after implementation)

**Implementation Details:**
(To be filled after implementation)

**Testing Performed:**
- Documentation review - Pending
- Markdown validation - Pending
